use std::io::Cursor;
use std::io::Read;
use std::io::Seek;

use crate::{
    errors::AffError,
    real_virtuality::core::{
        binrw_utils::read_8wvr_material_names,
        decompress_lzss_unk_size,
        types::{TransformMatrix, XYPair},
    },
};
use binrw::{until_eof, Endian};

use binrw::BinRead;

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(magic = b"8WVR")]
pub struct WVR8 {
    pub header: WVR8Header,

    #[br(count = header.terrain_grid_size.x * header.terrain_grid_size.y)]
    pub elevations: Vec<f32>,

    #[br(count = header.texture_grid_size.x * header.texture_grid_size.y)]
    pub material_indicies: Vec<u16>,

    pub rvmat_layer: RvmatLayer,

    #[br(parse_with = until_eof)]
    pub objects: Vec<Wvr8Object>,
}

impl WVR8 {
    pub fn from_read(reader: &mut (impl Read + Seek)) -> Result<Self, AffError> {
        let mut magic_buf = vec![0_u8; 4];
        reader.read_exact(&mut magic_buf)?;
        reader.rewind()?;
        if magic_buf != b"8WVR" {
            let data = decompress_lzss_unk_size(reader)?;

            let mut cursor = Cursor::new(data);
            let oprw = Self::read_wvr8(&mut cursor)?;
            return Ok(oprw);
        }
        let oprw = Self::read_wvr8(reader)?;
        Ok(oprw)
    }

    fn read_wvr8(reader: &mut (impl Read + Seek)) -> Result<Self, AffError> {
        let mut wvr8 = Self::read_options(reader, Endian::Little, ())?;

        // Remove last object because it's a dummy
        wvr8.objects.remove(wvr8.objects.len() - 1);

        Ok(wvr8)
    }
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
pub struct WVR8Header {
    pub texture_grid_size: XYPair,
    pub terrain_grid_size: XYPair,

    pub cell_size: f32,
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
pub struct RvmatLayer {
    material_count: u32,

    #[br(args_raw(material_count))]
    #[br(parse_with = read_8wvr_material_names)]
    pub materials: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
pub struct Wvr8Object {
    pub transform_matrix: TransformMatrix,
    pub object_id: u32,

    file_name_length: u32,
    #[br(count = file_name_length)]
    #[br(map = |s: Vec<u8>| String::from_utf8_lossy(&s).to_string())]
    p3d_file_name: String,
}
