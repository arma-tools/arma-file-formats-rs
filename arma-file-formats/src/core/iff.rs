//! IFF style containers used by Enfusion formats: a `FORM` header followed by chunks, each a tag
//! and a size. Tags and sizes are big endian, chunk contents are up to the format.

use std::io::{Read, Seek};

use binrw::{BinRead, binrw};

use crate::errors::AffError;

#[binrw]
#[brw(big, magic = b"FORM")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormHeader {
    /// Size of the form type and the chunks.
    pub size: u32,
    pub form_type: [u8; 4],
}

impl FormHeader {
    pub const SIZE: u32 = 12;

    /// Reads the header, [`AffError::InvalidFileError`] if it is not a `FORM`.
    pub fn read_form<R: Read + Seek>(reader: &mut R) -> Result<Self, AffError> {
        Self::read(reader).map_err(|e| match e {
            binrw::Error::BadMagic { .. } => AffError::InvalidFileError,
            e => e.into(),
        })
    }

    /// Position after the last chunk of a form that starts at `start`.
    pub fn end(self, start: u64) -> u64 {
        start + 8 + u64::from(self.size)
    }

    pub fn form_type_lossy(self) -> String {
        String::from_utf8_lossy(&self.form_type).into_owned()
    }
}

#[binrw]
#[brw(big)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkHeader {
    pub tag: [u8; 4],
    /// Size of the chunk contents, without this header.
    pub size: u32,
}

impl ChunkHeader {
    pub const SIZE: u32 = 8;
}
