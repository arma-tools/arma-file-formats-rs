use binrw::{BinRead, BinResult, NullString};
use std::io::SeekFrom;
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

    #[br(dbg)]
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

    for i in 0..count {
        println!("Lod Index: {}", i);
        reader.seek(SeekFrom::Start(start_address_of_lods[i].into()))?;
        lods.push(Lod::read_options(reader, endian, (args,))?);
    }

    Ok(lods)
}

#[derive(BinRead, Derivative)]
#[derivative(Debug, Default)]
pub enum Resolution {
    #[derivative(Default)]
    #[br(magic = 1f32)]
    Lod1,
    #[br(magic = 2f32)]
    Lod2,
    #[br(magic = 3f32)]
    Lod3,
    #[br(magic = 4f32)]
    Lod4,
    #[br(magic = 5f32)]
    Lod5,
    #[br(magic = 6f32)]
    Lod6,
    #[br(magic = 7f32)]
    Lod7,
    #[br(magic = 8f32)]
    Lod8,
    #[br(magic = 9f32)]
    Lod9,
    #[br(magic = 10f32)]
    Lod10,
    #[br(magic = 1E3f32)]
    ViewGunner,
    #[br(magic = 1.1E3f32)]
    ViewPilot,
    #[br(magic = 1.2E3f32)]
    ViewCargo,
    #[br(magic = 1.202E3f32)]
    Unk1,
    #[br(magic = 1E4f32)]
    ShadowVolume,
    #[br(magic = 1.001E4f32)]
    ShadowVolume2,
    #[br(magic = 1.1E4f32)]
    StencilShadow,
    #[br(magic = 1.101E4f32)]
    StencilShadow2,
    #[br(magic = 1.102E4f32)]
    Unk3,
    #[br(magic = 1E13f32)]
    Geometry,
    #[br(magic = 4E13f32)]
    Unk4,
    #[br(magic = 1E15f32)]
    Memory,
    #[br(magic = 2E15f32)]
    LandContact,
    #[br(magic = 3E15f32)]
    Roadway,
    #[br(magic = 4E15f32)]
    Paths,
    #[br(magic = 5E15f32)]
    HitPoints,
    #[br(magic = 6E15f32)]
    ViewGeometry,
    #[br(magic = 7E15f32)]
    FireGeometry,
    #[br(magic = 8E15f32)]
    ViewCargoGeometry,
    #[br(magic = 9E15f32)]
    ViewCargoFireGeometry,
    #[br(magic = 1E16f32)]
    ViewCommander,
    #[br(magic = 1.1E16f32)]
    ViewCommanderGeometry,
    #[br(magic = 1.2E16f32)]
    ViewCommanderFireGeometry,
    #[br(magic = 1.3E16f32)]
    ViewPilotGeometry,
    #[br(magic = 1.4E16f32)]
    ViewPilotFireGeometry,
    #[br(magic = 1.5E16f32)]
    ViewGunnerGeometry,
    #[br(magic = 1.6E16f32)]
    ViewGunnerFireGeometry,
    #[br(magic = 1.7E16f32)]
    SubParts,
    #[br(magic = 1.8E16f32)]
    ShadowVolumeViewCargo,
    #[br(magic = 1.9E16f32)]
    ShadowVolumeViewPilot,
    #[br(magic = 2E16f32)]
    ShadowVolumeViewGunner,
    #[br(magic = 2E16f32)]
    Unk2,
    #[br(magic = 2.1E16f32)]
    Unk5,
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
