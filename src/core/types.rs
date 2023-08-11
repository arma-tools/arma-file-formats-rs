use binrw::BinRead;
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(IntoPrimitive, TryFromPrimitive, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum PaaType {
    UNKNOWN = 0,
    DXT1 = 0xff01,
    DXT2 = 0xff02,
    DXT3 = 0xff03,
    DXT4 = 0xff04,
    DXT5 = 0xff05,
    RGBA4444 = 0x4444,
    RGBA5551 = 0x1555,
    RGBA8888 = 0x8888,
    GRAYwAlpha = 0x8080,
}

#[derive(Debug, Default, PartialEq, BinRead, Clone, Copy)]
pub struct BoundingBox {
    pub a: XY,
    pub b: XY,
    pub c: XY,
    pub d: XY,
}

#[derive(Debug, Default, PartialEq, BinRead, Clone, Copy)]
pub struct XY {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Default, PartialEq, BinRead, Clone, Copy)]
pub struct XYZTriplet {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Default, PartialEq, BinRead, Clone, Copy)]
pub struct STPair {
    pub s: XYZTriplet,
    pub t: XYZTriplet,
}

#[derive(Debug, Default, PartialEq, BinRead, Clone, Copy)]
pub struct TransformMatrix(
    pub XYZTriplet,
    pub XYZTriplet,
    pub XYZTriplet,
    pub XYZTriplet,
);

#[derive(Debug, Default, PartialEq, BinRead, Clone, Copy)]
pub struct D3DColorValue {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Default, BinRead, PartialEq, Eq, Clone)]
pub struct RGBAColor {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
}
