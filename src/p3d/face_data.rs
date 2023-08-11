use binrw::BinRead;

use super::ODOLArgs;

#[derive(Debug, Default, PartialEq, Clone, Copy, BinRead)]
#[br(import(args: ODOLArgs))]
pub struct FaceData {
    pub header_face_count: u32,
    pub color: u32,
    pub special: i32,
    pub or_hints: u32,

    #[br(if(args.version >= 39))]
    #[br(map = |x: u8| Some(x != 0))]
    pub has_skeleton: Option<bool>,

    #[br(if(args.version >= 51))]
    pub vertices_count: i32,
    #[br(if(args.version >= 51))]
    pub face_area: f32,
}
