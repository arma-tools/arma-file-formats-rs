use crate::real_virtuality::types::{RGBAColor, XYZTriplet};
use binrw::{BinRead, NullString};

use super::{skeleton::Skeleton, ODOLArgs};
use crate::real_virtuality::binrw_utils::read_compressed_array;
use derivative::Derivative;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(import(args: ODOLArgs, lod_count: u32))]
pub struct ModelInfo {
    pub index: u32,
    pub mem_lod_sphere: f32,
    pub geo_lod_sphere: f32,
    pub remarks: u32,
    pub and_hints: u32,
    pub or_hints: u32,
    pub aiming_center: XYZTriplet,
    pub map_icon_color: RGBAColor,
    pub map_selected_color: RGBAColor,
    pub view_density: f32,
    pub bbox_min_pos: XYZTriplet,
    pub bbox_max_pos: XYZTriplet,
    #[br(if(args.version >= 70))]
    pub lod_density_coef: Option<f32>,

    #[br(if(args.version >= 71))]
    pub draw_importance: Option<f32>,
    #[br(if(args.version >= 52))]
    pub bbox_min_visual: Option<XYZTriplet>,
    #[br(if(args.version >= 52))]
    pub bbox_max_visual: Option<XYZTriplet>,

    pub bounding_center: XYZTriplet,
    pub geometry_center: XYZTriplet,
    pub center_of_mass: XYZTriplet,

    #[br(count = 3)]
    pub inv_intertia: Vec<XYZTriplet>,
    #[br(map = |x: u8| x != 0)]
    pub auto_center: bool,
    #[br(map = |x: u8| x != 0)]
    pub lock_auto_center: bool,
    #[br(map = |x: u8| x != 0)]
    pub can_occlude: bool,
    #[br(map = |x: u8| x != 0)]
    pub can_be_occlude: bool,
    #[br(if(args.version >= 73))]
    #[br(map = |x: u8| Some(x != 0))]
    pub ai_covers: Option<bool>,

    #[br(if(args.version >= 42))]
    pub ht_min: Option<f32>,
    #[br(if(args.version >= 42))]
    pub ht_max: Option<f32>,
    #[br(if(args.version >= 42))]
    pub af_max: Option<f32>,
    #[br(if(args.version >= 42))]
    pub mf_max: Option<f32>,

    #[br(if(args.version >= 43))]
    pub m_fact: Option<f32>,
    #[br(if(args.version >= 43))]
    pub t_body: Option<f32>,

    #[br(if(args.version >= 33))]
    #[br(map = |x: u8| Some(x != 0))]
    pub force_not_alpha: Option<bool>,

    #[br(if(args.version >= 37))]
    pub sb_source: Option<SBSource>,
    #[br(if(args.version >= 37))]
    #[br(map = |x: u8| Some(x != 0))]
    pub prefer_shadow_volume: Option<bool>,

    #[br(if(args.version >= 48))]
    pub shadow_offset: Option<f32>,

    #[br(map = |x: u8| x != 0)]
    pub animated: bool,

    #[br(args(args.version))]
    pub skeleton: Skeleton,

    pub map_type: u8,

    #[br(args(4, args))]
    #[br(parse_with = read_compressed_array)]
    pub mass_array: Vec<f32>,

    pub mass: f32,
    pub mass_reciprocal: f32,
    pub alt_mass: f32,
    pub alt_mass_reciprocal: f32,

    #[br(if(args.version >= 72))]
    pub property_explosion_shielding: Option<f32>,

    #[br(if(args.version >= 53))]
    pub geometry_simple: Option<u8>,

    #[br(if(args.version >= 54))]
    pub geometry_phys: Option<u8>,

    pub memory: u8,
    pub geometry: u8,
    pub geometry_fire: u8,
    pub geometry_view: u8,
    pub geometry_view_pilot: u8,
    pub geometry_view_gunner: u8,
    pub unknown_signedbyte: i8,
    pub geometry_view_cargo: u8,
    pub land_contact: u8,
    pub roadway: u8,
    pub paths: u8,
    pub hitpoints: u8,
    pub min_shadow: u32,

    #[br(if(args.version >= 38))]
    #[br(map = |x: u8| Some(x != 0))]
    pub can_blend: Option<bool>,

    pub property_class: NullString,
    pub property_damage: NullString,

    #[br(map = |x: u8| x != 0)]
    pub property_frequent: bool,

    #[br(if(args.version >= 31))]
    pub unknown_int: u32,

    #[br(if(args.version >= 57))]
    #[br(count = lod_count)]
    pub prefferred_shadow_volumne_lod: Option<Vec<i32>>,
    #[br(if(args.version >= 57))]
    #[br(count = lod_count)]
    pub prefferred_shadow_buffer_lod: Option<Vec<i32>>,
    #[br(if(args.version >= 57))]
    #[br(count = lod_count)]
    pub prefferred_shadow_buffer_lod_vis: Option<Vec<i32>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
pub enum SBSource {
    #[derivative(Default)]
    #[br(magic = 0i32)]
    Visual = 0,
    #[br(magic = 1i32)]
    ShadowVolume = 1,
    #[br(magic = 2i32)]
    Explicit = 2,
    #[br(magic = 3i32)]
    None = 3,
    #[br(magic = 4i32)]
    VisualEx = 4,
}

impl ModelInfo {}

#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
#[br(repr = u8)]
pub enum MapType {
    MapTree = 0,
    MapSmallTree = 1,
    MapBush = 2,
    MapBuilding = 3,
    MapHouse = 4,
    MapForestBorder = 5,
    MapForestTriangle = 6,
    MapForestSquare = 7,
    MapChurch = 8,
    MapChapel = 9,
    MapCross = 10,
    MapRock = 11,
    MapBunker = 12,
    MapFortress = 13,
    MapFountain = 14,
    MapViewTower = 15,
    MapLighthouse = 16,
    MapQuay = 17,
    MapFuelstation = 18,
    MapHospital = 19,
    MapFence = 20,
    MapWall = 21,
    MapHide = 22,
    MapBusStop = 23,
    MapRoad = 24,
    MapForest = 25,
    MapTransmitter = 26,
    MapStack = 27,
    MapRuin = 28,
    MapTourism = 29,
    MapWatertower = 30,
    MapTrack = 31,
    MapMainRoad = 32,
    MapRocks = 33,
    MapPowerLines = 34,
    MapRailWay = 35,
    #[derivative(Default)]
    NMapTypes = 36,
}
