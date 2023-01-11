use deku::{bitvec::AsBits, DekuContainerRead, DekuRead};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek},
    path::Path,
};

use crate::{
    core::read::ReadExtTrait,
    errors::{RvffError, RvffOdolError},
    p3d::model_info::ModelInfo,
};

#[derive(Debug)]
pub struct ODOL {
    pub version: u32,

    use_lzo: bool,
    use_compression_flag: bool,

    pub app_id: u32,
    pub prefix: String,
    pub muzzle_flash: String,

    pub resolutions: Vec<f32>,

    pub model_info: ModelInfo,
}

impl ODOL {
    pub fn new() -> Self {
        ODOL {
            version: 0,
            use_lzo: false,
            use_compression_flag: false,
            app_id: 0,
            muzzle_flash: String::new(),
            prefix: String::new(),
            resolutions: Vec::new(),
            model_info: ModelInfo::default(),
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, RvffError> {
        let file = File::open(path)?;
        let mut buf_reader = BufReader::new(file);
        Self::from_stream(&mut buf_reader)
    }

    pub fn from_stream<R>(reader: &mut R) -> Result<Self, RvffError>
    where
        R: BufRead + Seek,
    {
        assert_eq!(&reader.read_string_lossy(4)?, "ODOL");

        let mut odol = Self::new();

        odol.version = reader.read_u32()?;

        if odol.version > 73 {
            return Err(RvffError::RvffOdolError(RvffOdolError::UnknownVersion(
                odol.version,
            )));
        } else if odol.version < 28 {
            return Err(RvffError::RvffOdolError(RvffOdolError::UnsupportedVersion(
                odol.version,
            )));
        }

        if odol.version >= 44 {
            odol.use_lzo = true;
        }
        if odol.version >= 64 {
            odol.use_compression_flag = true;
        }

        if odol.version == 58 {
            odol.prefix = reader.read_string_zt()?;
        }

        if odol.version >= 59 {
            odol.app_id = reader.read_u32()?;
        }

        if odol.version >= 58 {
            odol.muzzle_flash = reader.read_string_zt()?;
        }

        let lod_count = reader.read_u32()?;
        odol.resolutions.reserve(lod_count as usize);
        for _ in 0..lod_count {
            odol.resolutions.push(reader.read_f32()?);
        }

        let model_info_bytes = reader.read_bytes(16384)?;
        let (rest, model_info) =
            ModelInfo::read(model_info_bytes.as_bits(), (odol.version, lod_count)).unwrap();
        odol.model_info = model_info;
        Ok(odol)
    }
}

impl Default for ODOL {
    fn default() -> Self {
        Self::new()
    }
}
