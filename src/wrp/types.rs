use crate::core::types::{TransformMatrix, XYZTriplet};

use crate::core::deku_util::read_string_zt;
use crate::core::deku_util::write_string_zt;
use deku::{DekuContainerWrite, DekuRead, DekuUpdate, DekuWrite};

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub struct ObjectDeku {
    pub object_id: u32,
    pub model_index: u32, // 1 based...
    pub transform_matrx: TransformMatrix,
    #[deku(assert_eq = "0x02")]
    always_2: u32,
}

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub struct RoadNetDeku {
    #[deku(update = "self.road_parts.len()")]
    pub road_parts_count: u32,
    #[deku(count = "road_parts_count")]
    pub road_parts: Vec<RoadPartDeku>,
}

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub struct RoadPartDeku {
    #[deku(update = "self.road_positions.len()")]
    pub road_positions_count: u16,
    #[deku(count = "road_positions_count")]
    pub road_positions: Vec<XYZTriplet>,
    flags: u32,
    more_flags: u32,
    #[deku(
        reader = "read_string_zt(deku::rest)",
        writer = "write_string_zt(deku::output, &self.p3d_model)"
    )]
    pub p3d_model: String,
    pub transform_matrix: TransformMatrix,
}

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub struct ClassedModelDeku {
    #[deku(
        reader = "read_string_zt(deku::rest)",
        writer = "write_string_zt(deku::output, &self.class_name)"
    )]
    pub class_name: String,
    #[deku(
        reader = "read_string_zt(deku::rest)",
        writer = "write_string_zt(deku::output, &self.model_path)"
    )]
    pub model_path: String,
    pub position: XYZTriplet,
    unknown: u32,
}

#[derive(Eq, PartialEq, Debug, DekuRead, DekuWrite)]
pub struct TextureDeku {
    #[deku(
        reader = "read_string_zt(deku::rest)",
        writer = "write_string_zt(deku::output, &self.texture)"
    )]
    pub texture: String,
    #[deku(assert_eq = "0x00")]
    last_byte: u8,
}
