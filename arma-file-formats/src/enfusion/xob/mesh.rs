use binrw::binread;

use crate::enfusion::xob::{XobSectionHeader, XobUvSet, head::tangent_frame_count};

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
#[binread]
#[derive(Debug, Clone, Default)]
#[br(little, import(header: &XobSectionHeader))]
pub struct XobMesh {
    #[br(count = header.triangle_count as usize * 3)]
    pub indices: Vec<u16>,
    #[br(count = header.triangle_count as usize * 3)]
    pub tangent_indices: Vec<u16>,

    #[br(temp, args { count: usize::from(header.vertex_count), inner: (header.is_skinned(),) })]
    raw_positions: Vec<RawPosition>,
    #[br(calc = raw_positions.iter().map(|p| p.position).collect())]
    pub positions: Vec<[f32; 3]>,
    /// Index into `skin_weights` per vertex, empty for static meshes.
    #[br(calc = raw_positions.iter().filter_map(|p| p.skin_weight_index).collect())]
    pub skin_weight_indices: Vec<u32>,

    #[br(count = header.vertex_count, map = |v: Vec<u32>| v.into_iter().map(unpack_vector).collect())]
    pub normals: Vec<[f32; 3]>,
    /// RGBA, empty if the section has no vertex colors.
    #[br(if(header.has_colors()), count = header.vertex_count, map = bgra_to_rgba)]
    pub colors: Vec<[u8; 4]>,

    #[br(temp, count = usize::from(header.vertex_count) * header.uv_sets.len())]
    raw_uvs: Vec<[u16; 2]>,
    /// Texture coordinates indexed by `[uv_set][vertex]`.
    #[br(calc = unpack_uvs(&raw_uvs, &header.uv_sets))]
    pub uvs: Vec<Vec<[f32; 2]>>,

    #[br(temp, args {
        count: usize::from(header.tangent_vertex_count),
        inner: (tangent_frame_count(header.uv_sets.len()),),
    })]
    raw_tangent_vertices: Vec<RawTangentVertex>,
    #[br(calc = raw_tangent_vertices.iter().map(|v| v.vertex).collect())]
    pub tangent_vertex_map: Vec<u32>,
    /// Tangent frames indexed by `[uv_set][tangent vertex]`.
    #[br(calc = unpack_tangent_frames(&raw_tangent_vertices, tangent_frame_count(header.uv_sets.len())))]
    pub tangent_frames: Vec<Vec<XobTangentFrame>>,

    #[br(if(header.is_skinned()), count = header.skin_weight_count)]
    pub skin_weights: Vec<XobSkinWeight>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XobTangentFrame {
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
}

/// Three weights are stored, the fourth completes the sum to one.
#[binread]
#[derive(Debug, Clone, Copy, PartialEq)]
#[br(little)]
pub struct XobSkinWeight {
    /// Indices into the bone palette of the section.
    pub bones: [u8; 4],
    #[br(temp)]
    stored_weights: [f32; 3],
    #[br(calc = {
        let [a, b, c] = stored_weights;
        [a, b, c, (1.0 - a - b - c).max(0.0)]
    })]
    pub weights: [f32; 4],
}

#[binread]
#[br(little, import(skinned: bool))]
struct RawPosition {
    position: [f32; 3],
    #[br(if(skinned))]
    skin_weight_index: Option<u32>,
}

#[binread]
#[br(little, import(frame_count: usize))]
struct RawTangentVertex {
    vertex: u32,
    #[br(count = frame_count)]
    frames: Vec<[u32; 2]>,
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

fn bgra_to_rgba(colors: Vec<[u8; 4]>) -> Vec<[u8; 4]> {
    colors
        .into_iter()
        .map(|[b, g, r, a]| [r, g, b, a])
        .collect()
}

/// UVs are interleaved per vertex and normalized to the bounds of their set.
fn unpack_uvs(raw: &[[u16; 2]], sets: &[XobUvSet]) -> Vec<Vec<[f32; 2]>> {
    sets.iter()
        .enumerate()
        .map(|(index, set)| {
            raw.iter()
                .skip(index)
                .step_by(sets.len())
                .map(|&[u, v]| {
                    [
                        (set.u_max - set.u_min).mul_add(unpack_snorm16(u), set.u_min),
                        (set.v_max - set.v_min).mul_add(unpack_snorm16(v), set.v_min),
                    ]
                })
                .collect()
        })
        .collect()
}

fn unpack_tangent_frames(
    raw: &[RawTangentVertex],
    frame_count: usize,
) -> Vec<Vec<XobTangentFrame>> {
    (0..frame_count)
        .map(|frame| {
            raw.iter()
                .map(|vertex| {
                    let [tangent, bitangent] = vertex.frames[frame];
                    XobTangentFrame {
                        tangent: unpack_vector(tangent),
                        bitangent: unpack_vector(bitangent),
                    }
                })
                .collect()
        })
        .collect()
}
