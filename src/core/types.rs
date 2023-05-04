use binrw::BinRead;
use derivative::Derivative;
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

// impl XYZTriplet {
//     pub fn new() -> Self {
//         XYZTriplet {
//             x: 0.0,
//             y: 0.0,
//             z: 0.0,
//         }
//     }

//     pub fn from_reader<R>(reader: &mut R) -> Result<XYZTriplet, io::Error>
//     where
//         R: BufRead + Seek,
//     {
//         let mut xyz = XYZTriplet::new();
//         xyz.read(reader)?;
//         Ok(xyz)
//     }

//     pub fn read<R>(&mut self, reader: &mut R) -> Result<(), io::Error>
//     where
//         R: BufRead + Seek,
//     {
//         self.x = reader.read_f32()?;
//         self.y = reader.read_f32()?;
//         self.z = reader.read_f32()?;
//         Ok(())
//     }
// }

// use deku::{DekuContainerWrite, DekuRead, DekuUpdate, DekuWrite};

// #[derive(PartialEq, Debug, DekuRead, DekuWrite)]
// pub struct BoundingBox {
//     pub a: XY,
//     pub b: XY,
//     pub c: XY,
//     pub d: XY,
// }

#[derive(PartialEq, BinRead, Derivative, Clone, Copy)]
#[derivative(Debug, Default)]
pub struct BoundingBoxBinrw {
    pub a: XYBinrw,
    pub b: XYBinrw,
    pub c: XYBinrw,
    pub d: XYBinrw,
}

// #[derive(PartialEq, Debug, DekuRead, DekuWrite)]
// pub struct XY {
//     pub x: f32,
//     pub y: f32,
// }

#[derive(PartialEq, BinRead, Derivative, Clone, Copy)]
#[derivative(Debug, Default)]
pub struct XYBinrw {
    pub x: f32,
    pub y: f32,
}

// #[derive(PartialEq, DekuRead, DekuWrite, Derivative)]
// #[derivative(Debug, Default)]
// pub struct XYZTriplet {
//     x: f32,
//     y: f32,
//     z: f32,
// }

#[derive(PartialEq, BinRead, Derivative, Clone, Copy)]
#[derivative(Debug, Default)]
pub struct XYZTripletBinrw {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(PartialEq, BinRead, Derivative, Clone, Copy)]
#[derivative(Debug, Default)]
pub struct STPair {
    pub s: XYZTripletBinrw,
    pub t: XYZTripletBinrw,
}

// #[derive(PartialEq, Debug, DekuRead, DekuWrite)]
// pub struct TransformMatrix(XYZTriplet, XYZTriplet, XYZTriplet, XYZTriplet);

#[derive(PartialEq, BinRead, Derivative, Clone, Copy)]
#[derivative(Debug, Default)]
pub struct TransformMatrixBinrw(
    pub XYZTripletBinrw,
    pub XYZTripletBinrw,
    pub XYZTripletBinrw,
    pub XYZTripletBinrw,
);

#[derive(PartialEq, BinRead, Derivative, Clone, Copy)]
#[derivative(Debug, Default)]
pub struct D3DColorValue {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

// #[derive(PartialEq, Debug, DekuRead, DekuWrite)]
// pub(crate) struct BytesUntilZeroData {
//     #[deku(until = "|v: &u8| *v == 0")]
//     pub(crate) bytes: Vec<u8>,
// }

#[derive(BinRead, PartialEq, Derivative, Clone)]
#[derivative(Debug, Default)]
pub struct RGBAColor {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
}
