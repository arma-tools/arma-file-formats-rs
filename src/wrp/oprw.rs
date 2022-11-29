use std::io::Read;

use crate::core::types::XYZTriplet;
use crate::errors::RvffError;

use super::*;
use crate::core::deku_util::read_lzo;
use crate::core::deku_util::read_string_zt_vec;
use crate::core::deku_util::write_string_zt_vec;
use deku::DekuContainerRead;
use deku::{DekuContainerWrite, DekuRead, DekuUpdate, DekuWrite};

const OPRW_SIZE_OF_WPROBJECT: u32 = 60;

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
#[deku(magic = b"OPRW")]
pub struct Oprw {
    pub version: u32,
    #[deku(cond = "*version > 24")]
    pub app_id: Option<u32>,

    pub layer_size_x: u32,
    pub layer_size_y: u32,
    pub map_size_x: u32,
    pub map_size_y: u32,

    #[deku(skip, default = "*map_size_x * *map_size_y")]
    pub map_size: u32,
    #[deku(skip, default = "*layer_size_x * *layer_size_y")]
    pub layer_size: u32,
    pub layer_cell_size: f32,

    pub geography: QuadTree<u32>,
    pub cfg_env_sounds: QuadTree<u32>,

    #[deku(update = "self.peaks.len()")]
    pub peak_count: u32,
    #[deku(count = "peak_count")]
    pub peaks: Vec<XYZTriplet>,

    pub rvmat_layer_index: QuadTree<u32>,

    #[deku(
        reader = "read_lzo(*map_size, deku::rest)",
       // writer = "OprwDekuTest::write_quadtree(deku::output, &self.cell_env)"
    )]
    pub random_clutter: Vec<u8>,

    #[deku(
        reader = "read_lzo(*map_size, deku::rest)",
       // writer = "OprwDekuTest::write_quadtree(deku::output, &self.cell_env)"
    )]
    pub compressed_bytes: Vec<u8>,

    #[deku(
        reader = "read_lzo(*map_size*4, deku::rest)",
       // writer = "OprwDekuTest::write_quadtree(deku::output, &self.cell_env)"
    )]
    pub elevation: Vec<u8>,

    #[deku(update = "self.rvmats.len()")]
    pub rvmat_count: u32,
    #[deku(count = "rvmat_count")]
    pub rvmats: Vec<TextureDeku>,

    #[deku(update = "self.models.len()")]
    pub model_count: u32,
    #[deku(count = "model_count")]
    #[deku(
        reader = "read_string_zt_vec(deku::rest, *model_count as usize)",
        writer = "write_string_zt_vec(deku::output, &self.models)"
    )]
    pub models: Vec<String>,

    #[deku(update = "self.classes.len()")]
    pub classes_count: u32,
    #[deku(count = "classes_count")]
    pub classes: Vec<ClassedModelDeku>,

    pub unknown_grid_block_3: QuadTree<u32>,

    pub size_of_objects: u32,

    pub unknown_grid_block_4: QuadTree<u32>,

    pub size_of_map_info: u32,

    #[deku(
        reader = "read_lzo(*layer_size, deku::rest)",
       // writer = "OprwDekuTest::write_quadtree(deku::output, &self.cell_env)"
    )]
    pub compressed_bytes_2: Vec<u8>,

    #[deku(
        reader = "read_lzo(*map_size, deku::rest)",
       // writer = "OprwDekuTest::write_quadtree(deku::output, &self.cell_env)"
    )]
    pub compressed_bytes_3: Vec<u8>,

    pub max_object_id: u32,
    pub size_of_roadnets: u32,
    #[deku(count = "*layer_size")]
    pub road_nets: Vec<RoadNetDeku>,

    #[deku(count = "*size_of_objects/OPRW_SIZE_OF_WPROBJECT")]
    pub objects: Vec<ObjectDeku>,

    #[deku(
        reader = "read_map_info(*size_of_map_info, deku::rest)",
       // writer = "OprwDekuTest::write_quadtree(deku::output, &self.cell_env)"
    )]
    pub map_infos: Vec<MapInfoDeku>,
}

impl Oprw {
    pub fn new() -> Self {
        Oprw {
            version: 0,
            app_id: None,
            layer_size_x: 0,
            layer_size_y: 0,
            map_size_x: 0,
            map_size_y: 0,
            map_size: 0,
            layer_size: 0,
            layer_cell_size: 0.0,
            geography: Default::default(),
            cfg_env_sounds: Default::default(),
            peak_count: 0,
            peaks: Vec::new(),
            rvmat_layer_index: Default::default(),
            random_clutter: vec![],
            compressed_bytes: vec![],
            elevation: vec![],
            rvmat_count: 0,
            rvmats: vec![],
            model_count: 0,
            models: vec![],
            classes_count: 0,
            classes: vec![],
            unknown_grid_block_3: Default::default(),
            size_of_objects: 0,
            unknown_grid_block_4: Default::default(),
            size_of_map_info: 0,
            compressed_bytes_2: vec![],
            compressed_bytes_3: vec![],
            max_object_id: 0,
            size_of_roadnets: 0,
            road_nets: vec![],
            objects: vec![],
            map_infos: vec![],
        }
    }

    pub fn from_stream<R>(reader: &mut R) -> Result<Oprw, RvffError>
    where
        R: Read,
    {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf)?;
        let (_, mut oprw) = Oprw::from_bytes((&buf, 0))?;

        oprw.road_nets.retain(|x| x.road_parts_count > 0);
        Ok(oprw)
    }
}

impl Default for Oprw {
    fn default() -> Self {
        Self::new()
    }
}
