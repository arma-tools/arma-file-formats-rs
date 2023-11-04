use binrw::BinRead;

#[derive(Debug, Default, PartialEq, Clone, Copy, BinRead)]
pub struct BoundingBox {
    pub a: XY,
    pub b: XY,
    pub c: XY,
    pub d: XY,
}

#[derive(Debug, Default, PartialEq, Clone, Copy, BinRead)]
pub struct XY {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Default, PartialEq, Clone, Copy, BinRead)]
pub struct XYZTriplet {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Default, PartialEq, Clone, Copy, BinRead)]
pub struct STPair {
    pub s: XYZTriplet,
    pub t: XYZTriplet,
}

#[derive(Debug, Default, PartialEq, Clone, Copy, BinRead)]
pub struct TransformMatrix(
    pub XYZTriplet,
    pub XYZTriplet,
    pub XYZTriplet,
    pub XYZTriplet,
);

#[derive(Debug, Default, PartialEq, Clone, Copy, BinRead)]
pub struct D3DColorValue {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, BinRead)]
pub struct RGBAColor {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
}
