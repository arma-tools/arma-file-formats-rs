use crate::core::types::BoundingBox;

use deku::{
    bitvec::{BitSlice, Msb0},
    DekuContainerWrite, DekuEnumExt, DekuError, DekuRead, DekuUpdate, DekuWrite,
};

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
#[deku(type = "u32")]
pub enum MapInfoDeku {
    #[deku(id_pat = "0|1|2|10|11|12|13|14|15|16|17|22|23|26|27|30")] // 12 (cham)
    MapType1 {
        id: u32,
        object_id: u32,
        x: f32,
        y: f32,
    },
    #[deku(id_pat = "24|31|32")]
    MapType2 {
        id: u32,
        object_id: u32,
        bounds: BoundingBox,
    },
    #[deku(id_pat = "25|33|41|42|43")] // 41, 42, 43 (stratis)
    MapType3 {
        id: u32,
        color: u32,
        indicator: u32,
        #[deku(count = "4")]
        floats: Vec<f32>,
    },
    #[deku(id_pat = "3|4|8|9|18|19|20|21|28|29|36|37|38|39")]
    // 36 (malden), 37,38 (altis), 39 (stratis) no doc
    MapType4 {
        id: u32,
        object_id: u32,
        bounds: BoundingBox,
        #[deku(count = "4")]
        color: Vec<u8>,
    },
    #[deku(id_pat = "34")]
    MapType5 {
        id: u32,
        object_id: u32,
        #[deku(count = "4")]
        floats: Vec<f32>, // minimal bounding box???
    },
    #[deku(id_pat = "35")]
    MapType35 {
        id: u32,
        object_id: u32,
        #[deku(count = "6")]
        line: Vec<f32>,
        unknown: u8,
    },
}

pub(crate) fn read_map_info(
    size_of_map_info: u32,
    mut rest: &BitSlice<u8, Msb0>,
) -> Result<(&BitSlice<u8, Msb0>, Vec<MapInfoDeku>), DekuError> {
    let mut map_infos = Vec::with_capacity(size_of_map_info as usize);
    while !rest.is_empty() {
        let (rest_read, value) = MapInfoDeku::read(rest, ()).unwrap();
        rest = rest_read;
        map_infos.push(value);
    }

    Ok((rest, map_infos))
}
