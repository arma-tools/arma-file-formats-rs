use binrw::BinRead;
use binrw::NullString;
use derivative::Derivative;

use crate::real_virtuality::types::XYZTriplet;

use super::ODOLArgs;

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(import(args: ODOLArgs))]
pub struct Animations {
    pub animation_class_count: u32,

    #[br(count = animation_class_count)]
    #[br(args { inner: (args.version,) })]
    pub animation_classes: Vec<AnimationClass>,

    resolution_count: i32,

    #[br(count = resolution_count)]
    pub bones_2_anims: Vec<Bones2Anims>,

    #[br(count = resolution_count)]
    #[br(args { inner: (animation_class_count, animation_classes.clone(),) })]
    pub anims_2_bones: Vec<Anims2Bones>,
}

#[derive(Debug, Default, PartialEq, Clone, BinRead)]
#[br(import(version: u32))]
pub struct AnimationClass {
    pub anim_transform_type: AnimType,
    pub anim_class_name: NullString,
    pub anim_source: NullString,
    pub min_phase: f32,
    pub max_phase: f32,
    pub min_value: f32,
    pub max_value: f32,

    #[br(if(version >= 56))]
    pub anim_period: Option<f32>,
    #[br(if(version >= 56))]
    pub init_phase: Option<f32>,

    pub source_address: AnimAddress,

    #[br(if(
        anim_transform_type == AnimType::Rotation ||
        anim_transform_type == AnimType::RotationX ||
        anim_transform_type == AnimType::RotationY ||
        anim_transform_type == AnimType::RotationZ
    ))]
    pub angle_0: Option<f32>,
    #[br(if(
        anim_transform_type == AnimType::Rotation ||
        anim_transform_type == AnimType::RotationX ||
        anim_transform_type == AnimType::RotationY ||
        anim_transform_type == AnimType::RotationZ
    ))]
    pub angle_1: Option<f32>,

    #[br(if(
        anim_transform_type == AnimType::Translation ||
        anim_transform_type == AnimType::TranslationX ||
        anim_transform_type == AnimType::TranslationY ||
        anim_transform_type == AnimType::TranslationZ
    ))]
    pub offset_0: Option<f32>,
    #[br(if(
        anim_transform_type == AnimType::Translation ||
        anim_transform_type == AnimType::TranslationX ||
        anim_transform_type == AnimType::TranslationY ||
        anim_transform_type == AnimType::TranslationZ
    ))]
    pub offset_1: Option<f32>,

    #[br(if(anim_transform_type == AnimType::Direct))]
    pub axis_pos: Option<XYZTriplet>,
    #[br(if(anim_transform_type == AnimType::Direct))]
    pub axis_dir: Option<XYZTriplet>,
    #[br(if(anim_transform_type == AnimType::Direct))]
    pub axis_angle: Option<f32>,
    #[br(if(anim_transform_type == AnimType::Direct))]
    pub axis_offset: Option<f32>,

    #[br(if(anim_transform_type == AnimType::Hide))]
    pub hide_value: Option<f32>,
    #[br(if(anim_transform_type == AnimType::Hide && version >= 55))]
    pub unknown_hide: Option<f32>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
#[br(repr = u32)]
pub enum AnimType {
    #[derivative(Default)]
    Rotation = 0,
    RotationX = 1,
    RotationY = 2,
    RotationZ = 3,
    Translation = 4,
    TranslationX = 5,
    TranslationY = 6,
    TranslationZ = 7,
    Direct = 8,
    Hide = 9,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
#[br(repr = u32)]
pub enum AnimAddress {
    #[derivative(Default)]
    AnimClamp = 0,
    AnimLoop = 1,
    AnimMirror = 2,
    NAnimAddress = 3,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
pub struct Bones2Anims {
    bone_count: u32,

    #[br(count = bone_count)]
    pub bone_2_anim_class_list: Vec<Bone2AnimClassList>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
pub struct Bone2AnimClassList {
    anim_class_count: u32,

    #[br(count = anim_class_count)]
    pub animation_class_index: Vec<u32>,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Anims2Bones {
    pub animation_class_indices: Vec<AnimBones>,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub struct AnimBones {
    pub skeleton_bone_name_index: i32,

    pub axis_pos: Option<XYZTriplet>,
    pub axis_dir: Option<XYZTriplet>,
}

impl BinRead for Anims2Bones {
    type Args<'a> = (u32, Vec<AnimationClass>);

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let (animation_class_count, animation_classes) = args;
        let animation_class_count = animation_class_count as usize;
        let mut animation_class_indices = Vec::with_capacity(animation_class_count);

        for anim_class in animation_classes {
            let skeleton_bone_name_index = i32::read_options(reader, endian, ())?;
            if skeleton_bone_name_index != -1
                && anim_class.anim_transform_type != AnimType::Direct
                && anim_class.anim_transform_type != AnimType::Hide
            {
                let axis_pos = XYZTriplet::read_options(reader, endian, ())?;
                let axis_dir = XYZTriplet::read_options(reader, endian, ())?;
                animation_class_indices.push(AnimBones {
                    skeleton_bone_name_index,
                    axis_pos: Some(axis_pos),
                    axis_dir: Some(axis_dir),
                });
            } else {
                animation_class_indices.push(AnimBones {
                    skeleton_bone_name_index,
                    axis_pos: None,
                    axis_dir: None,
                });
            }
        }

        Ok(Self {
            animation_class_indices,
        })
    }
}
