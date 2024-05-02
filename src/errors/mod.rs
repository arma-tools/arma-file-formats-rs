use std::{io, string::FromUtf8Error};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PaaError {
    #[error("No mipmaps were set")]
    NoMipmapError,

    #[error("Invalid mipmap at index `{0}`")]
    InvalidMipmapError(usize),

    #[error("IO failed")]
    PaaIOError(#[from] io::Error),

    #[error("Paa lzo failed")]
    PaaLzoErr(#[from] lzokay_native::Error),

    #[error("LZSS Error")]
    RvffLzssError(#[from] RvffLzssError),

    #[error("Invalid state")]
    InvalidState,

    #[error("unknown decoding error")]
    Unknown,
}

#[derive(Debug, Error)]
pub enum RvffLzssError {
    #[error("IO failed")]
    IOError(#[from] io::Error),

    #[error("LZSS Checksum Missmatch")]
    ChecksumMissmatch,

    #[error("LZSS Overflow")]
    Overflow,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Error)]
pub enum RvffOdolError {
    #[error("Signature Missing")]
    SignatureMissing,

    #[error("Unknown Version: `{0}`")]
    UnknownVersion(u32),

    #[error("Unsupported Version: `{0}`")]
    UnsupportedVersion(u32),
}

#[derive(Debug, Error)]
pub enum RvffError {
    #[error("IO failed {0}")]
    RvffIOError(#[from] io::Error),

    #[error("FromUTF8 failed {0}")]
    RvffUTFError(#[from] FromUtf8Error),

    #[error("Binrw failed {0}")]
    RvffBinrwError(#[from] binrw::Error),

    #[error("LZSS Error")]
    RvffLzssError(#[from] RvffLzssError),

    #[error("ODOL Error")]
    RvffOdolError(#[from] RvffOdolError),

    #[error("Invalid file")]
    InvalidFileError,

    #[error("PBO Entry {0} not found")]
    PboEntryNotFound(String),

    #[error("unknown decoding error")]
    Unknown,

    #[error("Parsing failed: {0}")]
    RvffParseError(String),
}
