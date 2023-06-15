use binrw::{BinRead, BinResult, NullString};
use byteorder::ReadBytesExt;
use std::collections::HashMap;
use std::io::{Cursor, Read, SeekFrom};
use std::marker::PhantomData;
use std::{
    fs::File,
    io::{BufReader, Seek},
    path::Path,
};

use crate::core::decompress_lzss_unk_size;
use crate::{errors::RvffError, p3d::model_info::ModelInfo};
use derivative::Derivative;

use super::animations::Animations;
use super::face_data::FaceData;
use super::lod::Lod;

#[derive(Derivative)]
#[derivative(Debug, Default, Copy, Clone)]
pub struct ODOLArgs {
    pub version: u32,
    pub use_lzo: bool,
    pub use_compression_flag: bool,
    pub skip_lods: bool,
}

#[derive(Derivative)]
#[derivative(Debug, Default, Copy, Clone)]
pub struct ODOLOptions {
    pub skip_lods: bool,
}

#[derive(BinRead, Derivative, Clone)]
#[derivative(Debug, Default)]
#[br(magic = b"ODOL")]
#[br(import(options: ODOLOptions))]
pub struct ODOL {
    #[br(assert((28..=73).contains(&version), "ODOL Version {} Unsupported", version))]
    pub version: u32,

    #[br(calc = version >= 44)]
    use_lzo: bool,

    #[br(calc = version >= 64)]
    use_compression_flag: bool,

    #[br(calc = ODOLArgs{ version, use_lzo, use_compression_flag, skip_lods: options.skip_lods })]
    args: ODOLArgs,

    #[br(if(version >= 59))]
    pub app_id: u32,
    #[br(if(version >= 58))]
    pub p3d_prefix: Option<NullString>,

    pub lod_count: u32,

    #[br(count = lod_count)]
    pub resolutions: Vec<Resolution>,

    #[br(args(args, lod_count))]
    pub model_info: ModelInfo,

    #[br(if(version >= 30))]
    #[br(map = |x: u8| Some(x != 0))]
    pub has_anims: Option<bool>,

    #[br(args(args))]
    #[br(if(version >= 30 && has_anims.unwrap_or_default()))]
    pub animations: Option<Animations>,

    #[br(count = lod_count)]
    pub start_address_of_lods: Vec<u32>,

    #[br(count = lod_count)]
    pub end_address_of_lods: Vec<u32>,

    #[br(count = lod_count)]
    #[br(map = |x: Vec<u8>| x.into_iter().map(|b| b!= 0).collect())]
    pub use_defaults: Vec<bool>,

    #[br(count = lod_count)]
    #[br(args { inner: (args,) })]
    #[br(map = |x: Vec<FaceData>| {
        let mut bool_iter = x.into_iter();
        use_defaults.iter().map(|def| {
            if *def {
                bool_iter.next()
            } else {
               None
            }
        }).collect()
    })]
    pub face_defaults: Vec<Option<FaceData>>,

    #[br(args(lod_count as usize, &start_address_of_lods, args,))]
    #[br(parse_with = read_lods)]
    pub lods: Vec<Lod>,
}

#[binrw::parser(reader, endian)]
pub(crate) fn read_lods(
    count: usize,
    start_address_of_lods: &[u32],
    args: ODOLArgs,
) -> BinResult<Vec<Lod>> {
    if args.skip_lods {
        return Ok(Vec::new());
    }

    let mut lods = Vec::with_capacity(count);

    #[allow(clippy::needless_range_loop)]
    for i in 0..count {
        println!("Lod Index: {}", i);
        reader.seek(SeekFrom::Start(start_address_of_lods[i].into()))?;
        lods.push(Lod::read_options(reader, endian, (args,))?);
    }

    Ok(lods)
}

#[derive(PartialEq, BinRead, Derivative, Clone)]
#[derivative(Debug, Default)]
pub struct Resolution {
    pub value: f32,

    #[br(args { value })]
    pub res: ResolutionEnum,
}

#[allow(illegal_floating_point_literal_pattern)]
#[derive(BinRead, Derivative, PartialEq, Clone)]
#[derivative(Debug, Default)]
#[br(import { value: f32 })]
pub enum ResolutionEnum {
    #[br(pre_assert(value < 1E3f32))]
    GraphicalLod,
    #[br(pre_assert(value == 1E3f32))]
    ViewGunner,
    #[br(pre_assert(value == 1.1E3f32))]
    ViewPilot,
    #[br(pre_assert(value == 1.2E3f32))]
    ViewCargo,
    #[br(pre_assert(value == 1.202E3f32))]
    ViewUnknown,
    #[br(pre_assert(value == 1E4f32))]
    ShadowVolume,
    #[br(pre_assert(value == 1.001E4f32))]
    ShadowVolume2,
    #[br(pre_assert(value == 1.1E4f32))]
    StencilShadow,
    #[br(pre_assert(value == 1.101E4f32))]
    StencilShadow2,
    #[br(pre_assert(value == 1.102E4f32))]
    StencilShadowUnknown,
    #[br(pre_assert(value == 1E13f32))]
    Geometry,
    #[br(pre_assert(value == 4E13f32))]
    Unknown4E13,
    #[br(pre_assert(value == 1E15f32))]
    Memory,
    #[br(pre_assert(value == 2E15f32))]
    LandContact,
    #[br(pre_assert(value == 3E15f32))]
    Roadway,
    #[br(pre_assert(value == 4E15f32))]
    Paths,
    #[br(pre_assert(value == 5E15f32))]
    HitPoints,
    #[br(pre_assert(value == 6E15f32))]
    ViewGeometry,
    #[br(pre_assert(value == 7E15f32))]
    FireGeometry,
    #[br(pre_assert(value == 8E15f32))]
    ViewCargoGeometry,
    #[br(pre_assert(value == 9E15f32))]
    ViewCargoFireGeometry,
    #[br(pre_assert(value == 1E16f32))]
    ViewCommander,
    #[br(pre_assert(value == 1.1E16f32))]
    ViewCommanderGeometry,
    #[br(pre_assert(value == 1.2E16f32))]
    ViewCommanderFireGeometry,
    #[br(pre_assert(value == 1.3E16f32))]
    ViewPilotGeometry,
    #[br(pre_assert(value == 1.4E16f32))]
    ViewPilotFireGeometry,
    #[br(pre_assert(value == 1.5E16f32))]
    ViewGunnerGeometry,
    #[br(pre_assert(value == 1.6E16f32))]
    ViewGunnerFireGeometry,
    #[br(pre_assert(value == 1.7E16f32))]
    SubParts,
    #[br(pre_assert(value == 1.8E16f32))]
    ShadowVolumeViewCargo,
    #[br(pre_assert(value == 1.9E16f32))]
    ShadowVolumeViewPilot,
    #[br(pre_assert(value == 2E16f32))]
    ShadowVolumeViewGunner,
    #[br(pre_assert(value == 2.1E16f32))]
    Wreck,
    #[br(pre_assert(true))]
    #[derivative(Default)]
    Unknown(PhantomData<f32>),
}

impl ODOL {
    pub fn new() -> Self {
        ODOL::default()
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, RvffError> {
        let file = File::open(path)?;
        let mut buf_reader = BufReader::new(file);
        Self::from_stream(&mut buf_reader)
    }

    pub(crate) fn from_stream_lazy<R>(reader: &mut R) -> Result<Self, RvffError>
    where
        R: Read + Seek,
    {
        ODOL::read(reader, true)
    }

    pub fn from_stream<R>(reader: &mut R) -> Result<Self, RvffError>
    where
        R: Read + Seek,
    {
        ODOL::read(reader, false)
    }

    fn read<R>(reader: &mut R, skip_lods: bool) -> Result<Self, RvffError>
    where
        R: Read + Seek,
    {
        let opt = ODOLOptions { skip_lods };

        let mut magic_buf = vec![0_u8; 4];
        reader.read_exact(&mut magic_buf)?;
        reader.rewind()?;
        if magic_buf != b"ODOL" {
            reader.read_u8()?;
            reader.read_exact(&mut magic_buf)?;
            if magic_buf == b"ODOL" {
                let data = decompress_lzss_unk_size(reader)?;
                let mut cursor = Cursor::new(data);

                return Ok(ODOL::read_le_args(&mut cursor, (opt,))?);
            }
        }
        Ok(ODOL::read_le_args(reader, (opt,))?)
    }

    pub fn read_lod<RS>(
        &self,
        reader: &mut RS,
        resolution: ResolutionEnum,
    ) -> Result<Lod, RvffError>
    where
        RS: Read + Seek,
    {
        if let Some(lod_index) = self.resolutions.iter().position(|r| r.res == resolution) {
            if let Some(start_address) = self.start_address_of_lods.get(lod_index) {
                reader.seek(SeekFrom::Start((*start_address).into()))?;
                let lod = Lod::read_le_args(reader, (self.args,))?;
                //self.lods.insert(resolution, lod);
                // doesnt handle lzss btw

                return Ok(lod);
            }
        }

        todo!()
    }
}

pub struct OdolLazyReader<R>
where
    R: Read + Seek,
{
    reader: R,
    pub odol: ODOL,
    pub lods: HashMap<ResolutionEnum, Lod>,
}

impl<R> OdolLazyReader<R>
where
    R: Read + Seek,
{
    pub fn from_reader(mut reader: R) -> Result<OdolLazyReader<R>, RvffError> {
        let odol = ODOL::from_stream_lazy(&mut reader)?;
        Ok(OdolLazyReader {
            lods: HashMap::new(),
            reader,
            odol,
        })
    }

    pub fn read_lod(&mut self, resolution: ResolutionEnum) -> Result<Lod, RvffError> {
        if let Some(lod_index) = self
            .odol
            .resolutions
            .iter()
            .position(|r| r.res == resolution)
        {
            if let Some(start_address) = self.odol.start_address_of_lods.get(lod_index) {
                self.reader.seek(SeekFrom::Start((*start_address).into()))?;
                let lod = Lod::read_le_args(&mut self.reader, (self.odol.args,))?;
                //self.lods.insert(resolution, lod);

                return Ok(lod);
            }
        }

        todo!()
    }
}
