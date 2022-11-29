use std::io::{self, BufRead, Seek};

use crate::core::read::ReadExtTrait;
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

impl XYZTriplet {
    pub fn new() -> Self {
        XYZTriplet {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn from_reader<R>(reader: &mut R) -> Result<XYZTriplet, io::Error>
    where
        R: BufRead + Seek,
    {
        let mut xyz = XYZTriplet::new();
        xyz.read(reader)?;
        Ok(xyz)
    }

    pub fn read<R>(&mut self, reader: &mut R) -> Result<(), io::Error>
    where
        R: BufRead + Seek,
    {
        self.x = reader.read_f32()?;
        self.y = reader.read_f32()?;
        self.z = reader.read_f32()?;
        Ok(())
    }
}

impl Default for XYZTriplet {
    fn default() -> Self {
        Self::new()
    }
}

use deku::{DekuContainerWrite, DekuRead, DekuUpdate, DekuWrite};

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub struct BoundingBox {
    pub a: XY,
    pub b: XY,
    pub c: XY,
    pub d: XY,
}

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub struct XY {
    pub x: f32,
    pub y: f32,
}

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub struct XYZTriplet {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub struct TransformMatrix(XYZTriplet, XYZTriplet, XYZTriplet, XYZTriplet);

#[derive(PartialEq, Debug, DekuRead, DekuWrite)]
pub(crate) struct BytesUntilZeroData {
    #[deku(until = "|v: &u8| *v == 0")]
    pub(crate) bytes: Vec<u8>,
}
