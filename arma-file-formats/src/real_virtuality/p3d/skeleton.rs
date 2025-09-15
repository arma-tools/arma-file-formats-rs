use binrw::{BinRead, NullString};

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
#[br(import(version: u32))]
pub struct Skeleton {
    pub name: NullString,

    #[br(if(!(*name).is_empty() && version >= 23))]
    #[br(map = |x: u8| Some(x != 0))]
    pub is_discrete: Option<bool>,

    #[br(if(!(*name).is_empty(), 0))]
    pub bone_names_count: u32,
    #[br(count = bone_names_count)]
    pub skeleton_bones: Vec<Bone>,

    #[br(if(!(*name).is_empty() && version > 40))]
    pub pivots_name_obsolete: Option<NullString>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
pub struct Bone {
    pub bone_name: NullString,
    pub bone_parent: NullString,
}
