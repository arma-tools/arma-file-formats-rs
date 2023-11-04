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
