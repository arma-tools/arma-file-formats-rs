use crate::core::types::RGBAColor;
use crate::core::types::XYZTriplet;
use deku::DekuContainerRead;
use deku::DekuContainerWrite;
use deku::DekuEnumExt;
use deku::DekuError;
use deku::DekuUpdate;
use deku::{DekuRead, DekuWrite};
use derivative::Derivative;

use super::skeleton::Skeleton;

#[derive(PartialEq, DekuRead, DekuWrite, Derivative)]
#[derivative(Debug, Default)]
#[deku(ctx = "version: u32, lod_count: u32")]
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

    #[deku(cond = "version >= 70")]
    pub lod_density_coef: Option<f32>,

    #[deku(cond = "version >= 71")]
    pub draw_importance: Option<f32>,

    #[deku(cond = "version >= 52")]
    pub bbox_min_visual: Option<XYZTriplet>,

    #[deku(cond = "version >= 52")]
    pub bbox_max_visual: Option<XYZTriplet>,

    pub bounding_center: XYZTriplet,
    pub geometry_center: XYZTriplet,
    pub center_of_mass: XYZTriplet,

    #[deku(count = "3")]
    pub inv_intertia: Vec<XYZTriplet>,
    pub auto_center: bool,
    pub lock_auto_center: bool,
    pub can_occlude: bool,
    pub can_be_occlude: bool,
    #[deku(cond = "version >= 73")]
    pub ai_covers: Option<bool>,

    #[deku(cond = "version >= 42")]
    pub ht_min: Option<f32>,
    #[deku(cond = "version >= 42")]
    pub ht_max: Option<f32>,
    #[deku(cond = "version >= 42")]
    pub af_max: Option<f32>,
    #[deku(cond = "version >= 42")]
    pub mf_max: Option<f32>,

    #[deku(cond = "version >= 43")]
    pub m_fact: Option<f32>,
    #[deku(cond = "version >= 43")]
    pub t_body: Option<f32>,

    #[deku(cond = "version >= 33")]
    pub force_not_alpha: Option<bool>,

    #[deku(cond = "version >= 37")]
    pub sb_source: Option<SBSource>,
    #[deku(cond = "version >= 37")]
    pub prefer_shadow_volume: Option<bool>,

    #[deku(cond = "version >= 48")]
    pub shadow_offset: Option<f32>,

    pub animated: bool,

    #[deku(ctx = "version")]
    pub skeleton: Skeleton,
}

#[allow(clippy::enum_variant_names, non_camel_case_types)]
#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
#[deku(type = "i32")]
pub enum SBSource {
    SBS_Visual = 0,
    SBS_ShadowVolume = 1,
    SBS_Explicit = 2,
    SBS_None = 3,
    SBS_VisualEx = 4,
}

impl ModelInfo {}
