//! Real Virtuality skeletal animations (`.rtm`).
//!
//! Two formats share the extension, both little endian:
//!
//! - [`Rtm`], `RTM_0101`: what Oxygen, Object Builder and `FBXToRTM` export. The step
//!   (`3 f32`), the frame and bone counts (`u32`), the bone names (32 bytes each, NUL
//!   terminated), then per frame its phase (`f32`) and per bone its name again and a 3x4
//!   matrix (`12 f32`). An `RTM_MDAT` block with metadata values and keystones (animation
//!   events like `StepSound`) can come first. The older `RTM_0100` stores only the z of the
//!   step.
//! - [`Bmtr`], `BMTR`: what Binarize makes of it and the game ships. Version, step, frame
//!   count, bone names (NUL terminated), metadata and keystones, then the frame phases and
//!   per frame one transform per bone, a quaternion of 4 `i16` and a translation of 3 half
//!   floats. The phases and each frame's transforms are arrays with a `u32` count. In
//!   version 5 a compression flag follows, `0` stored as is, `2` LZO1X compressed. Versions
//!   3 and 4 have no flag and compress arrays of 1024 bytes or more, version 3 has no
//!   metadata and keystones.
//!
//! [`Bmtr::from_rtm`] binarizes like Binarize, [`Bmtr::to_rtm`] goes back. Binarize turns
//! the transforms by 180 degrees around y (negates x and z), see [`Bmtr::flipped_xz`].
//!
//! Arma 3 has 3,827 `BMTR` files, all version 5, and 39 `RTM_0101` files. About 2,150 mod
//! folders add 8,200 `BMTR` (27 of version 3, 38 of version 4) and 1,900 `RTM_0101` files.
//! All of them are read here. Plain files, version 4 and all version 5 files of the game
//! round trip byte exact: version 5 arrays are compressed with Binarize's own LZO1X
//! compressor, reimplemented. Written version 3 files hold the same animation, packed a few
//! bytes differently.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::real_virtuality::rtm::RtmFile;
//!
//! let file = RtmFile::from(&mut BufReader::new(File::open("amovpercmrunslowwlnrdb.rtm")?))?;
//! if let RtmFile::Binarized(rtm) = file {
//!     println!("{} bones, {} frames", rtm.bones.len(), rtm.frames.len());
//!     for event in &rtm.keystones {
//!         println!("{} at {}", event.name, event.time);
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod bmtr;
mod convert;
mod io;
mod lzo;
#[allow(clippy::module_inception)]
mod rtm;

use std::io::{Read, Seek, Write};

pub use self::{bmtr::*, rtm::*};

use self::{bmtr::BMTR_SIGNATURE, io::read_exact};
use crate::errors::AffError;

/// An `.rtm` of either format.
#[derive(Debug, Clone, PartialEq)]
pub enum RtmFile {
    /// `RTM_0101` or `RTM_0100`, with or without `RTM_MDAT`.
    Plain(Rtm),
    /// `BMTR`
    Binarized(Bmtr),
}

impl RtmFile {
    /// Reads either format, by its signature.
    pub fn from<R: Read + Seek>(input: &mut R) -> Result<Self, AffError> {
        let head: [u8; 4] = read_exact(input)?;
        if &head == BMTR_SIGNATURE {
            return Ok(Self::Binarized(Bmtr::read_body(input)?));
        }
        let tail: [u8; 4] = read_exact(input)?;
        let mut signature = [0; 8];
        signature[..4].copy_from_slice(&head);
        signature[4..].copy_from_slice(&tail);
        Ok(Self::Plain(Rtm::read_after_signature(signature, input)?))
    }

    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        match self {
            Self::Plain(rtm) => rtm.write(out),
            Self::Binarized(rtm) => rtm.write(out),
        }
    }
}
