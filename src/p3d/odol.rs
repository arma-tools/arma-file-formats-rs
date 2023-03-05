use binrw::{BinRead, BinResult, NullString};
use std::io::SeekFrom;
use std::marker::PhantomData;
use std::{
    fs::File,
    io::{BufRead, BufReader, Seek},
    path::Path,
};

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
}

#[derive(BinRead, Derivative)]
#[derivative(Debug, Default)]
#[br(magic = b"ODOL")]
pub struct ODOL {
    #[br(assert((28..=73).contains(&version), "ODOL Version {} Unsupported", version))]
    pub version: u32,

    #[br(calc = version >= 44)]
    use_lzo: bool,

    #[br(calc = version >= 64)]
    use_compression_flag: bool,

    #[br(calc = ODOLArgs{ version, use_lzo, use_compression_flag })]
    args: ODOLArgs,

    #[br(if(version == 58))]
    pub prefix: Option<NullString>,
    #[br(if(version > 59))]
    pub app_id: u32,
    #[br(if(version >= 58))]
    pub muzzle_flash: Option<NullString>,

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

    //#[br(count = lod_count)]
    // #[br(count = 1)]
    #[br(args(lod_count as usize, &start_address_of_lods, args,))]
    #[br(parse_with = read_lods)]
    pub lods: Vec<Lod>,
}

#[binrw::parser(reader, endian)]
//fn read_compressed_array<T: for<'a> BinRead<Args<'a> = ()>, Copy>(
pub(crate) fn read_lods(
    count: usize,
    start_address_of_lods: &[u32],
    args: ODOLArgs,
) -> BinResult<Vec<Lod>> {
    let mut lods = Vec::with_capacity(count);

    #[allow(clippy::needless_range_loop)]
    for i in 0..count {
        println!("Lod Index: {}", i);
        reader.seek(SeekFrom::Start(start_address_of_lods[i].into()))?;
        lods.push(Lod::read_options(reader, endian, (args,))?);
    }

    Ok(lods)
}

#[derive(PartialEq, BinRead, Derivative)]
#[derivative(Debug, Default)]
pub struct Resolution {
    pub value: f32,

    #[br(args { value })]
    pub res: ResolutionEnum,
}

#[allow(illegal_floating_point_literal_pattern)]
#[derive(BinRead, Derivative, PartialEq)]
#[derivative(Debug, Default)]
#[br(import { value: f32 })]
//#[br(repr(f32))]
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

    pub fn from_stream<R>(reader: &mut R) -> Result<Self, RvffError>
    where
        R: BufRead + Seek,
    {
        Ok(ODOL::read_le(reader).unwrap())

        // assert_eq!(&reader.read_string_lossy(4)?, "ODOL");

        // let mut odol = Self::new();

        // odol.version = reader.read_u32()?;

        // if odol.version > 73 {
        //     return Err(RvffError::RvffOdolError(RvffOdolError::UnknownVersion(
        //         odol.version,
        //     )));
        // } else if odol.version < 28 {
        //     return Err(RvffError::RvffOdolError(RvffOdolError::UnsupportedVersion(
        //         odol.version,
        //     )));
        // }

        // if odol.version >= 44 {
        //     odol.use_lzo = true;
        // }
        // if odol.version >= 64 {
        //     odol.use_compression_flag = true;
        // }

        // if odol.version == 58 {
        //     odol.prefix = reader.read_string_zt()?;
        // }

        // if odol.version >= 59 {
        //     odol.app_id = reader.read_u32()?;
        // }

        // if odol.version >= 58 {
        //     odol.muzzle_flash = reader.read_string_zt()?;
        // }

        // let lod_count = reader.read_u32()?;
        // odol.resolutions.reserve(lod_count as usize);
        // for _ in 0..lod_count {
        //     odol.resolutions.push(reader.read_f32()?);
        // }

        // let model_info_bytes = reader.read_bytes(16384)?;
        // let (rest, model_info) =
        //     ModelInfo::read(model_info_bytes.as_bits(), (odol.version, lod_count)).unwrap();
        // odol.model_info = model_info;
        // Ok(odol)
    }
}
