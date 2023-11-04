mod mipmap;
#[allow(clippy::module_inception)]
mod paa;
mod tagg;

use num_enum::{IntoPrimitive, TryFromPrimitive};

pub use self::mipmap::Mipmap;
pub use self::paa::Paa;
pub use self::tagg::Tagg;

#[derive(Debug, PartialEq, Eq, Clone, Copy, IntoPrimitive, TryFromPrimitive)]
#[repr(u16)]
pub enum PaaType {
    Unknown = 0,
    Dxt1 = 0xff01,
    Dxt2 = 0xff02,
    Dxt3 = 0xff03,
    Dxt4 = 0xff04,
    Dxt5 = 0xff05,
    Rgba4444 = 0x4444,
    Rgba5551 = 0x1555,
    Rgba8888 = 0x8888,
    GrayWAlpha = 0x8080,
}
