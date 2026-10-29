use std::io::{Cursor, Read, Seek};

use crate::{
    core::read::ReadExtTrait,
    enfusion::xob::{XobSectionHeader, head::tangent_frame_count},
    errors::AffError,
};

/// Decoded geometry of one LOD section.
///
/// The section stores the same triangles twice, over two vertex spaces:
/// * `indices` are triangles over the vertices (`positions`, `normals`, `colors`, `uvs`).
///   Vertices are numbered in order of first use and carry no tangents.
/// * `tangent_indices` are the triangles over the tangent vertices, in a vertex cache
///   optimized order. Every tangent vertex references a vertex through `tangent_vertex_map`
///   and adds one tangent frame per UV set; vertices are split where their tangent frames
///   differ.
///
/// Mapping `tangent_indices` through `tangent_vertex_map` yields exactly the triangles of
/// `indices` (same winding), only in a different order.
#[derive(Debug, Clone, Default)]
pub struct XobMesh {
    pub indices: Vec<u16>,
    pub tangent_indices: Vec<u16>,
    pub positions: Vec<[f32; 3]>,
    /// Index into `skin_weights` per vertex, empty for static meshes.
    pub skin_weight_indices: Vec<u32>,
    pub normals: Vec<[f32; 3]>,
    /// RGBA, empty if the section has no vertex colors.
    pub colors: Vec<[u8; 4]>,
    /// Texture coordinates indexed by `[uv_set][vertex]`.
    pub uvs: Vec<Vec<[f32; 2]>>,
    pub tangent_vertex_map: Vec<u32>,
    /// Tangent frames indexed by `[uv_set][tangent vertex]`.
    pub tangent_frames: Vec<Vec<XobTangentFrame>>,
    pub skin_weights: Vec<XobSkinWeight>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XobTangentFrame {
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XobSkinWeight {
    /// Indices into the bone palette of the section.
    pub bones: [u8; 4],
    pub weights: [f32; 4],
}

impl XobMesh {
    pub(crate) fn read(
        input: &mut Cursor<&[u8]>,
        header: &XobSectionHeader,
    ) -> Result<Self, AffError> {
        let index_count = header.triangle_count as usize * 3;
        let vertex_count = usize::from(header.vertex_count);
        let tangent_vertex_count = usize::from(header.tangent_vertex_count);

        let indices = read_u16s(input, index_count)?;
        let tangent_indices = read_u16s(input, index_count)?;

        let mut positions = Vec::with_capacity(vertex_count);
        let mut skin_weight_indices = Vec::new();
        for _ in 0..vertex_count {
            positions.push(read_vec3(input)?);
            if header.is_skinned() {
                skin_weight_indices.push(input.read_u32()?);
            }
        }

        let normals = (0..vertex_count)
            .map(|_| input.read_u32().map(unpack_vector))
            .collect::<Result<_, _>>()?;

        let colors = if header.has_colors() {
            (0..vertex_count)
                .map(|_| {
                    let mut bgra = [0; 4];
                    input
                        .read_exact(&mut bgra)
                        .map(|()| [bgra[2], bgra[1], bgra[0], bgra[3]])
                })
                .collect::<Result<_, _>>()?
        } else {
            Vec::new()
        };

        let mut uvs = vec![Vec::with_capacity(vertex_count); header.uv_sets.len()];
        for _ in 0..vertex_count {
            for (set, uv) in header.uv_sets.iter().zip(uvs.iter_mut()) {
                let u = unpack_snorm16(input.read_u16()?);
                let v = unpack_snorm16(input.read_u16()?);
                uv.push([
                    (set.u_max - set.u_min).mul_add(u, set.u_min),
                    (set.v_max - set.v_min).mul_add(v, set.v_min),
                ]);
            }
        }

        let frame_count = tangent_frame_count(header.uv_sets.len());
        let mut tangent_vertex_map = Vec::with_capacity(tangent_vertex_count);
        let mut tangent_frames = vec![Vec::with_capacity(tangent_vertex_count); frame_count];
        for _ in 0..tangent_vertex_count {
            tangent_vertex_map.push(input.read_u32()?);
            for frames in &mut tangent_frames {
                frames.push(XobTangentFrame {
                    tangent: unpack_vector(input.read_u32()?),
                    bitangent: unpack_vector(input.read_u32()?),
                });
            }
        }

        let skin_weights = if header.is_skinned() {
            (0..header.skin_weight_count)
                .map(|_| read_skin_weight(input))
                .collect::<Result<_, _>>()?
        } else {
            Vec::new()
        };

        Ok(Self {
            indices,
            tangent_indices,
            positions,
            skin_weight_indices,
            normals,
            colors,
            uvs,
            tangent_vertex_map,
            tangent_frames,
            skin_weights,
        })
    }
}

/// Unpacks a vector stored as 11:11:10 bits (x in the top bits).
#[must_use]
pub fn unpack_vector(packed: u32) -> [f32; 3] {
    let x = packed >> 21;
    let y = (packed >> 10) & 0x7FF;
    let z = packed & 0x3FF;
    [
        x as f32 / 1023.0 - 1.0,
        y as f32 / 1023.0 - 1.0,
        z as f32 / 511.0 - 1.0,
    ]
}

/// Maps a signed normalized 16 bit value onto `[0, 1]`.
fn unpack_snorm16(value: u16) -> f32 {
    (f32::from(value as i16) / 32767.0).mul_add(0.5, 0.5)
}

fn read_u16s<I: Read + Seek>(input: &mut I, count: usize) -> Result<Vec<u16>, AffError> {
    Ok((0..count)
        .map(|_| input.read_u16())
        .collect::<Result<_, _>>()?)
}

fn read_vec3<I: Read + Seek>(input: &mut I) -> Result<[f32; 3], AffError> {
    Ok([input.read_f32()?, input.read_f32()?, input.read_f32()?])
}

/// Three weights are stored, the fourth completes the sum to one.
fn read_skin_weight<I: Read + Seek>(input: &mut I) -> Result<XobSkinWeight, AffError> {
    let mut bones = [0; 4];
    input.read_exact(&mut bones)?;
    let [a, b, c] = read_vec3(input)?;
    Ok(XobSkinWeight {
        bones,
        weights: [a, b, c, (1.0 - a - b - c).max(0.0)],
    })
}
