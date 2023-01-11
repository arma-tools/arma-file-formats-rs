use crate::core::deku_util::read_string_zt;
use crate::core::deku_util::read_string_zt_opt;
use crate::core::deku_util::write_string_zt;
use crate::core::deku_util::write_string_zt_opt_str;
use crate::core::types::RGBAColor;
use crate::core::types::XYZTriplet;
use deku::DekuContainerRead;
use deku::DekuContainerWrite;
use deku::DekuEnumExt;
use deku::DekuError;
use deku::DekuUpdate;
use deku::{DekuRead, DekuWrite};
use derivative::Derivative;

#[derive(PartialEq, DekuRead, DekuWrite, Derivative)]
#[derivative(Debug, Default)]
#[deku(ctx = "version: u32")]
pub struct Skeleton {
    #[deku(
        reader = "read_string_zt(deku::rest)",
        writer = "write_string_zt(deku::output, &self.name)"
    )]
    pub name: String,

    #[deku(cond = "!(*name).is_empty() && version >= 23")]
    pub is_discrete: Option<bool>,

    #[deku(update = "self.skeleton_bones.len()")]
    pub bone_names_count: u32,
    #[deku(count = "bone_names_count")]
    pub skeleton_bones: Vec<Bone>,
    // #[deku(
    //     cond = "!(*name).is_empty() && version >= 23",
    //     reader = "read_string_zt_opt(deku::rest)",
    //     writer = "write_string_zt_opt_str(deku::output, &self.pivots_name_obsolete)"
    // )]
    // pub pivots_name_obsolete: Option<String>,
}

#[derive(PartialEq, DekuRead, DekuWrite, Derivative)]
#[derivative(Debug, Default)]
pub struct Bone {
    #[deku(
        reader = "read_string_zt(deku::rest)",
        writer = "write_string_zt(deku::output, &self.bone_name)"
    )]
    pub bone_name: String,
    #[deku(
        reader = "read_string_zt(deku::rest)",
        writer = "write_string_zt(deku::output, &self.bone_parent)"
    )]
    pub bone_parent: String,
}
