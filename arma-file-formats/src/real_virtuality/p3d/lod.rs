use crate::real_virtuality::{
    binrw_utils::{
        decompress_array, read_compressed, read_compressed_array, read_compressed_size_cond,
        read_condensed_array_cond, read_normals_parse, read_st_parse, read_vertex_index_array,
    },
    types::{D3DColorValue, STPair, TransformMatrix, XYZTriplet},
};
use std::{collections::HashMap, ops::Range};

use binrw::{BinRead, BinResult, NullString};

use super::ODOLArgs;

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(import(args: ODOLArgs))]
pub struct Lod {
    pub proxy_count: i32,

    #[br(count = proxy_count)]
    #[br(args { inner: (args,) })]
    pub proxies: Vec<Proxy>,

    #[allow(clippy::struct_field_names)]
    lod_item_count: u32,
    #[br(count = lod_item_count)]
    pub lod_items: Vec<u32>,

    bone_link_count: u32,

    #[br(count = bone_link_count)]
    pub bone_links: Vec<BoneLink>,

    #[br(if(args.version >= 50))]
    pub vertex_count: Option<u32>,

    #[br(args(args.version < 50, 4, args))]
    #[br(parse_with = read_condensed_array_cond)]
    pub clip_old_format: Option<Vec<i32>>,

    #[br(if(args.version >= 51))]
    pub face_area: Option<f32>,

    pub or_hints: i32,
    pub and_hints: i32,
    pub b_min: XYZTriplet,
    pub b_max: XYZTriplet,
    pub b_center: XYZTriplet,
    pub b_radius: f32,

    texture_count: u32,

    #[br(count = texture_count)]
    pub textures: Vec<NullString>,

    material_count: u32,

    #[br(count = material_count)]
    pub materials: Vec<LodMaterial>,

    #[br(args(args))]
    pub lod_edges: LodEdges,

    face_count: u32,
    offset_to_sections: u32,
    always_zero: u16,

    #[br(count = face_count)]
    #[br(args { inner: (args,) })]
    pub faces: Vec<LodFace>,

    section_count: u32,
    #[br(count = section_count)]
    #[br(args { inner: (args,) })]
    pub sections: Vec<LodSection>,

    named_selection_count: u32,

    #[br(count = named_selection_count)]
    #[br(args { inner: (args,) })]
    pub named_selection: Vec<LodNameSelection>,

    named_properties_count: u32,
    #[br(count = named_properties_count)]
    pub named_properties: Vec<LodNamedProperty>,

    frame_count: u32,
    #[br(count = frame_count)]
    pub frames: Vec<LodFrame>,

    pub icon_color: u32,
    pub selected_color: u32,
    pub special: u32,

    #[br(map = |x: u8| x != 0)]
    pub vertex_bone_ref_is_simple: bool,

    pub size_of_rest_data: u32,

    #[br(args(args.version >= 50, 4, args))]
    #[br(parse_with = read_condensed_array_cond)]
    pub clip: Option<Vec<u32>>,

    #[br(args(args))]
    pub default_uv_set: UVSet,

    #[br(map = |x: u32| if x > 0 { x - 1 } else { x })]
    uv_set_count: u32,

    #[br(count = uv_set_count)]
    #[br(args { inner: (args,) })]
    pub uv_sets: Vec<UVSet>,

    #[br(args(12, args,))]
    #[br(parse_with = read_compressed_array)]
    pub vertices: Vec<XYZTriplet>,

    #[br(args(args))]
    #[br(parse_with = read_normals_parse)]
    pub normals: Vec<XYZTriplet>,

    #[br(args(args))]
    #[br(parse_with = read_st_parse)]
    pub st_coords: Vec<STPair>,

    #[br(args(12, args,))]
    #[br(parse_with = read_compressed_array)]
    pub vertex_bone_ref: Vec<AnimationRTWeight>,

    #[br(args(32, args,))]
    #[br(parse_with = read_compressed_array)]
    pub neighbour_bone_ref: Vec<VertexNeighbour>,

    #[br(if(args.version >= 67))]
    unk_end: Option<u32>,

    #[br(if(args.version >= 68))]
    unk_end_2: Option<u8>,
}

impl Lod {
    /// The faces of each of [`Lod::sections`], as ranges of [`Lod::faces`]. `None` for a section
    /// whose offsets don't fall between faces.
    ///
    /// Sections follow each other without gaps. Checked against the test models (versions 40 to
    /// 75) and all models of Arma 3.
    #[must_use]
    pub fn section_faces(&self) -> Vec<Option<Range<usize>>> {
        let index_size = match self.sections.first() {
            Some(section) if section.short_indices => 2,
            _ => 4,
        };
        // Face index by offset, including the end of the last face.
        let mut offsets = HashMap::with_capacity(self.faces.len() + 1);
        let mut offset = 0;
        for (index, face) in self.faces.iter().enumerate() {
            offsets.insert(offset, index);
            offset += index_size * (1 + face.vertex_indices.len());
        }
        offsets.insert(offset, self.faces.len());

        self.sections
            .iter()
            .map(|section| {
                let start = *offsets.get(&(section.face_lower_index as usize))?;
                let end = *offsets.get(&(section.face_upper_index as usize))?;
                (start <= end).then_some(start..end)
            })
            .collect()
    }
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(import(args: ODOLArgs))]
pub struct Proxy {
    pub proxy_model: NullString,
    pub transofrmation: TransformMatrix,
    pub sequence_id: i32,
    pub named_selection_index: i32,
    pub bone_index: i32,

    #[br(if(args.version >= 40))]
    pub section_index: i32,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
pub struct BoneLink {
    link_count: u32,

    #[br(count = link_count)]
    pub values: Vec<u32>,
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
pub struct LodMaterial {
    pub material_name: NullString,

    pub version: u32,

    pub emissive: D3DColorValue,

    pub ambient: D3DColorValue,

    pub diffuse: D3DColorValue,

    pub forced_diffuse: D3DColorValue,

    pub specular: D3DColorValue,

    pub specular_2: D3DColorValue,

    pub specular_power: f32,

    pub pixel_shader: i32,

    pub vertex_shader: i32,
    pub main_light: i32,
    pub fog_mode: i32,

    #[br(if(version == 3))]
    #[br(map = |x: u8| Some(x != 0))]
    unk_bool: Option<bool>,

    #[br(if(version >= 6))]
    pub surface_file: Option<NullString>,

    #[br(if(version >= 4))]
    pub n_render_flags: Option<u32>,
    #[br(if(version >= 4))]
    pub render_flags: Option<u32>,

    #[br(if(version > 6))]
    pub texture_count: u32,

    #[br(if(version > 8))]
    pub transform_count: u32,

    #[br(count = texture_count)]
    #[br(args { inner: (version,) })]
    pub stage_textures: Vec<StageTexture>,

    #[br(count = transform_count)]
    pub stage_transforms: Vec<StageTransform>,

    #[br(if(version >= 10))]
    #[br(args(version,))]
    pub dummy_stage_textures: Option<StageTexture>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
#[br(import(mat_version: u32))]
pub struct StageTexture {
    #[br(if(mat_version >= 5))]
    pub render_flags: Option<u32>,

    pub texture: NullString,

    #[br(if(mat_version >= 8))]
    pub stage_id: Option<u32>,

    #[br(if(mat_version >= 11))]
    #[br(map = |x: u8| Some(x != 0))]
    pub use_world_env: Option<bool>,
}

#[derive(Debug, Default, PartialEq, Clone, Copy, BinRead)]
pub struct StageTransform {
    pub uv_source: u32,
    pub transformation: TransformMatrix,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
#[br(import(args: ODOLArgs))]
pub struct LodEdges {
    #[br(args_raw(args))]
    pub mlod_index: CompressedVertexIndexArray,

    #[br(args_raw(args))]
    pub vertex_index: CompressedVertexIndexArray,
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct CompressedVertexIndexArray {
    pub edges: Vec<u32>,
}

impl BinRead for CompressedVertexIndexArray {
    type Args<'a> = ODOLArgs;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let count = u32::read_options(reader, endian, ())? as usize;
        let edges: Vec<u32> = if args.version >= 69 {
            decompress_array::<u32>(reader, endian, 4, count, args)?
        } else {
            decompress_array::<u16>(reader, endian, 2, count, args)?
                .into_iter()
                .map(u32::from)
                .collect()
        };

        Ok(Self { edges })
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
#[br(import(args: ODOLArgs))]
pub struct LodFace {
    pub face_type: u8,

    #[br(args(args, face_type as usize))]
    #[br(parse_with = read_vertex_index_array)]
    pub vertex_indices: Vec<u32>,
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(import(args: ODOLArgs))]
pub struct LodSection {
    #[br(calc = args.version < 69)]
    pub short_indices: bool,

    /// Start and end of the section's faces in [`Lod::faces`] as byte offsets, with the vertex
    /// count and each vertex index of a face taking the size of an index: 4 bytes from version
    /// 69 (a triangle takes 16 bytes), 2 before. See [`Lod::section_faces`] for face indices.
    pub face_lower_index: u32,
    pub face_upper_index: u32,

    pub min_bone_index: u32,
    pub bone_count: u32,

    pub common_point_user_value: u32,
    pub common_texture_index: i16,
    pub common_face_flag: u32,
    pub material_index: i32,

    #[br(if(material_index == -1))]
    pub material: Option<NullString>,

    #[br(if(args.version >= 36, 1))]
    pub stage_count: u32,

    #[br(count = stage_count)]
    pub stages: Option<Vec<f32>>,

    #[br(if(args.version >= 67))]
    #[br(map = |x: i32| x >= 1)]
    pub has_unk_floats: bool,

    /// 11 floats, not a 4x3 matrix. Only seen on sections of collimator sights (ACO, ARCO, ...)
    /// in Arma 3, the flag before them was always 0 or 1.
    #[br(if(args.version >= 67 && has_unk_floats))]
    pub unk_floats: Option<[f32; 11]>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
#[br(import(args: ODOLArgs))]
pub struct LodNameSelection {
    pub name: NullString,

    #[br(args_raw(args))]
    pub selected_faces: CompressedVertexIndexArray,

    always_0: u32,

    #[br(map = |x: u8| x != 0)]
    pub is_sectional: bool,

    #[br(args(4, args))]
    #[br(parse_with = read_compressed_array)]
    pub vertex_indices: Vec<i32>,

    #[br(args_raw(args))]
    pub selected_vertices: CompressedVertexIndexArray,

    #[br(args(1, args))]
    #[br(parse_with = read_compressed)]
    pub selected_vertices_weights: Vec<u8>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
pub struct LodNamedProperty {
    pub property: NullString,
    pub value: NullString,
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
pub struct LodFrame {
    pub frame_time: f32,
    pub bone_count: u32,

    #[br(count = bone_count)]
    pub bone_positions: Vec<XYZTriplet>,
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(import(args: ODOLArgs))]
pub struct UVSet {
    #[br(calc = args.version >= 45)]
    pub is_discretized: bool,

    #[br(if(args.version >= 45))]
    pub min_u: Option<f32>,

    #[br(if(args.version >= 45))]
    pub min_v: Option<f32>,

    #[br(if(args.version >= 45))]
    pub max_u: Option<f32>,

    #[br(if(args.version >= 45))]
    pub max_v: Option<f32>,

    vertices_count: u32,

    #[br(map = |x: u8| x != 0)]
    pub default_fill: bool,

    #[br(calc = if args.version >= 45 { 4 } else { 8 })]
    value_size: u32,

    #[br(if(default_fill))]
    #[br(count = value_size)]
    pub default_value: Option<Vec<u8>>,

    #[br(args(!default_fill, value_size as usize, vertices_count as usize, args))]
    #[br(parse_with = read_compressed_size_cond)]
    pub uv_data: Option<Vec<u8>>,
}

impl UVSet {
    /// The coordinates, one `[u, v]` per vertex.
    ///
    /// Discretized sets (version 45 and up) store each coordinate as an `i16` spread over the
    /// set's `min..=max` range, older ones store `f32`s.
    #[must_use]
    pub fn uvs(&self) -> Vec<[f32; 2]> {
        let count = self.vertices_count as usize;
        let data = match (&self.uv_data, &self.default_value) {
            (Some(data), _) => data.as_slice(),
            (None, Some(value)) => value.as_slice(),
            (None, None) => return vec![[0.0; 2]; count],
        };
        let size = self.value_size as usize;
        let decode = |value: &[u8]| -> [f32; 2] {
            if self.is_discretized {
                // In f64: ranges can be thousands wide, f32 put values off by 1e-4.
                let range = |raw: [u8; 2], min: Option<f32>, max: Option<f32>| {
                    let (min, max) = (f64::from(min.unwrap_or(0.0)), f64::from(max.unwrap_or(0.0)));
                    let step = (f64::from(i16::from_le_bytes(raw)) + 32767.0) / 65534.0;
                    step.mul_add(max - min, min) as f32
                };
                [
                    range([value[0], value[1]], self.min_u, self.max_u),
                    range([value[2], value[3]], self.min_v, self.max_v),
                ]
            } else {
                let float = |raw: &[u8]| f32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
                [float(&value[0..4]), float(&value[4..8])]
            }
        };

        if self.uv_data.is_none() {
            return data
                .get(..size)
                .map_or_else(|| vec![[0.0; 2]; count], |v| vec![decode(v); count]);
        }
        data.chunks_exact(size).take(count).map(decode).collect()
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
pub struct AnimationRTWeight {
    pub small_count: i32,

    #[br(count = 8)]
    pub small_space: Vec<u8>,

    #[br(calc = {
        let small_count = small_count as usize;
        let mut res = Vec::with_capacity(small_count);
        for i in 0..small_count {
            res.push(AnimationRTPair { selection_index: small_space[i * 2], weight: small_space[i * 2 + 1] });
        }
        res
    })]
    pub animation_rt_pairs: Vec<AnimationRTPair>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, BinRead)]
pub struct AnimationRTPair {
    pub selection_index: u8,
    pub weight: u8,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
pub struct VertexNeighbour {
    pub pos_a: u16,
    unk_pos: u16,

    pub rtw_a: AnimationRTWeight,

    pub pos_b: u16,
    unk_pos_2: u16,

    pub rtw_b: AnimationRTWeight,
}
