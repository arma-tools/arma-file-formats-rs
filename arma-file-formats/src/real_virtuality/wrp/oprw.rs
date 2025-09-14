use std::io::Cursor;
use std::io::Read;
use std::io::Seek;

use super::QuadTree;
use crate::core::decompress_lzss_unk_size;
use crate::{
    errors::AffError,
    real_virtuality::binrw_utils::{read_compressed_array_count, read_compressed_data_cond_count},
    real_virtuality::{
        p3d::ODOLArgs,
        types::{BoundingBox, TransformMatrix, XYZTriplet, XY},
    },
};
use binrw::{until_eof, Endian, NullString};

use binrw::BinRead;
use derivative::Derivative;

const OPRW_SIZE_OF_WPROBJECT: u32 = 60;

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(magic = b"OPRW")]
pub struct OPRW {
    #[br(assert(version >= 10, "OPRW Version {} Unsupported", version))]
    pub version: u32,

    #[br(calc = version >= 23)]
    use_lzo: bool,

    #[br(calc = ODOLArgs{ version, use_lzo, use_compression_flag: false, skip_lods: false })]
    args: ODOLArgs,

    #[br(if(version >= 25))]
    pub app_id: Option<u32>,

    #[br(if(version >= 12))]
    pub layer_size_x: Option<u32>,
    #[br(if(version >= 12))]
    pub layer_size_y: Option<u32>,
    #[br(if(version >= 12))]
    pub map_size_x: Option<u32>,
    #[br(if(version >= 12))]
    pub map_size_y: Option<u32>,
    #[br(if(version >= 12))]
    pub layer_cell_size: Option<f32>,

    #[br(calc = map_size_x.and_then(|x| map_size_y.map(|y| x * y)))]
    pub map_size: Option<u32>,

    #[br(calc = layer_size_x.and_then(|x| layer_size_y.map(|y| x * y)))]
    pub layer_size: Option<u32>,

    #[br(args(2))]
    geography: QuadTree,

    #[br(args(1))]
    sound_map: QuadTree,

    mountain_count: u32,
    #[br(count = mountain_count)]
    pub mountains: Vec<XYZTriplet>,

    #[br(args(4))]
    rvmat_layer_index: QuadTree,

    #[br(args(version < 21, layer_size.map(|ls| ls * 2).unwrap_or_default() as usize, args,))]
    #[br(parse_with = read_compressed_data_cond_count)]
    pub random_clutter: Option<Vec<u8>>,

    #[br(args(version >= 18, map_size.unwrap_or_default() as usize, args,))]
    #[br(parse_with = read_compressed_data_cond_count)]
    pub grass: Option<Vec<u8>>,

    #[br(args(version >= 22, map_size.unwrap_or_default() as usize, args,))]
    #[br(parse_with = read_compressed_data_cond_count)]
    pub tex_index: Option<Vec<u8>>,

    #[br(args(4, map_size.unwrap_or_default() as usize, args,))]
    #[br(parse_with = read_compressed_array_count)]
    pub elevation: Vec<f32>,

    texture_count: u32,

    #[br(count = texture_count)]
    pub texures: Vec<Texture>,

    model_count: u32,
    #[br(count = model_count)]
    pub models: Vec<NullString>,

    #[br(if(version >= 15))]
    classed_model_count: Option<u32>,
    #[br(if(version >= 15))]
    #[br(count = classed_model_count.unwrap_or_default())]
    pub classed_models: Option<Vec<ClassedModel>>,

    #[br(args(4))]
    pub object_offsets: QuadTree,

    size_of_objects: u32,

    #[br(args(4))]
    pub map_object_offsets: QuadTree,

    size_of_map_info: u32,

    #[br(args(1, layer_size.unwrap_or_default() as usize, args,))]
    #[br(parse_with = read_compressed_array_count)]
    unknown_bytes_0: Vec<u8>,

    #[br(args(1, map_size.unwrap_or_default() as usize, args,))]
    #[br(parse_with = read_compressed_array_count)]
    unknown_bytes_1: Vec<u8>,

    pub max_object_id: u32,

    road_net_size: u32,

    #[br(count = layer_size.unwrap_or_default())]
    #[br(args { inner: (version,) })]
    pub road_net: Vec<RoadNet>,

    #[br(count = size_of_objects / OPRW_SIZE_OF_WPROBJECT)]
    #[br(args { inner: (version,) })]
    pub objects: Vec<Object>,
    //#[br(count = road_part_count)]
    #[br(parse_with = until_eof)]
    pub map_infos: Vec<MapInfo>,
}

impl OPRW {
    pub fn from_read(reader: &mut (impl Read + Seek)) -> Result<Self, AffError> {
        // OPRW
        let mut magic_buf = vec![0_u8; 4];
        reader.read_exact(&mut magic_buf)?;
        reader.rewind()?;
        if magic_buf != b"OPRW" {
            let data = decompress_lzss_unk_size(reader)?;

            let mut cursor = Cursor::new(data);
            let oprw = Self::read_oprw(&mut cursor)?;
            return Ok(oprw);
        }
        let oprw = Self::read_oprw(reader)?;
        Ok(oprw)
    }

    fn read_oprw(reader: &mut (impl Read + Seek)) -> Result<Self, AffError> {
        let mut oprw = Self::read_options(reader, Endian::Little, ())?;
        oprw.road_net.retain(|rn| rn.road_part_count > 0);
        Ok(oprw)
    }
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
pub struct ClassedModel {
    pub class_name: NullString,
    pub model_path: NullString,
    pub pos: XYZTriplet,
    pub obj_id: u32,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
pub struct Texture {
    pub texture_filename: NullString,
    flag: NullString,
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(import(version: u32))]
pub struct RoadNet {
    road_part_count: u32,

    #[br(count = road_part_count)]
    #[br(args { inner: (version,) })]
    pub road_parts: Vec<RoadPart>,
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(import(version: u32))]
pub struct RoadPart {
    road_pos_count: u16,

    #[br(count = road_pos_count)]
    pub positions: Vec<XYZTriplet>,

    #[br(if(version >= 24))]
    #[br(count = road_pos_count)]
    pub types: Option<Vec<u8>>,

    pub object_id: u32,

    #[br(if(version >= 16))]
    pub p3d_path: Option<NullString>,

    #[br(if(version >= 16))]
    pub transform_matrix: Option<TransformMatrix>,
}

#[derive(Debug, Default, PartialEq, Clone, Copy, BinRead)]
#[br(import(version: u32))]
pub struct Object {
    pub object_id: u32,
    pub model_index: u32,
    pub transform_matrx: TransformMatrix,

    #[br(if(version >= 14))]
    pub shape_params: Option<u32>,
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
pub struct MapInfo {
    pub id: u32,
    #[br(args(id))]
    pub data: MapData,
}

const MAP_TYPE_1_IDS: [u32; 16] = [0, 1, 2, 10, 11, 12, 13, 14, 15, 16, 17, 22, 23, 26, 27, 30]; // 12 (cham)
const MAP_TYPE_2_IDS: [u32; 3] = [24, 31, 32];
const MAP_TYPE_3_IDS: [u32; 5] = [25, 33, 41, 42, 43]; // 41, 42, 43 (stratis)
const MAP_TYPE_4_IDS: [u32; 14] = [3, 4, 8, 9, 18, 19, 20, 21, 28, 29, 36, 37, 38, 39]; // 36 (malden), 37,38 (altis), 39 (stratis) no doc

#[derive(Debug, PartialEq, Clone, BinRead, Derivative)]
#[derivative(Default)]
#[br(import(id: u32))]
pub enum MapData {
    #[br(pre_assert(MAP_TYPE_1_IDS.contains(&id)))]
    MapType1 { object_id: u32, x: f32, y: f32 },
    #[br(pre_assert(MAP_TYPE_2_IDS.contains(&id)))]
    MapType2 { object_id: u32, bounds: BoundingBox },
    #[br(pre_assert(MAP_TYPE_3_IDS.contains(&id)))]
    MapType3 {
        color: u32,
        indicator: u32,
        #[br(count = 4)]
        floats: Vec<f32>,
    },
    #[br(pre_assert(MAP_TYPE_4_IDS.contains(&id)))]
    MapType4 {
        object_id: u32,
        bounds: BoundingBox,
        #[br(count = 4)]
        color: Vec<u8>,
    },
    #[br(pre_assert(id == 34))]
    MapType5 {
        object_id: u32,
        #[br(count = 4)]
        floats: Vec<f32>, // minimal bounding box???
    },
    #[br(pre_assert(id == 35))]
    MapType35 {
        object_id: u32,
        #[br(count = 6)]
        line: Vec<f32>,
        unknown: u8,
    },

    #[br(pre_assert(id == 44))]
    MapTypeRiver {
        object_id: u32,
        polygon_count: u32,
        #[br(count = polygon_count)]
        polygon: Vec<XY>,
    },

    #[derivative(Default)]
    Unknown {
        #[br(dbg)]
        first_byte: u8,

        #[br(calc = if true { panic!("Unknown map type") } else { 0 })] // bruh
        _panics: u8,
    },
}
