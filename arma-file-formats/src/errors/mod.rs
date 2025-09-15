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
    LzssError(#[from] LzssError),

    #[error("Invalid state")]
    InvalidState,

    #[error("unknown decoding error")]
    Unknown,
}

#[derive(Debug, Error)]
pub enum LzssError {
    #[error("IO failed")]
    IOError(#[from] io::Error),

    #[error("LZSS Checksum Missmatch")]
    ChecksumMissmatch,

    #[error("LZSS Overflow")]
    Overflow,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Error)]
pub enum OdolError {
    #[error("Signature Missing")]
    SignatureMissing,

    #[error("Unknown Version: `{0}`")]
    UnknownVersion(u32),

    #[error("Unsupported Version: `{0}`")]
    UnsupportedVersion(u32),
}

#[derive(Debug, Error)]
pub enum AffError {
    #[error("IO failed {0}")]
    IOError(#[from] io::Error),

    #[error("FromUTF8 failed {0}")]
    UTFError(#[from] FromUtf8Error),

    #[error("Binrw failed {0}")]
    BinrwError(#[from] binrw::Error),

    #[error("LZSS Error")]
    LzssError(#[from] LzssError),

    #[error("ODOL Error")]
    OdolError(#[from] OdolError),

    #[error("Invalid file")]
    InvalidFileError,

    #[error("PBO Entry {0} not found")]
    PboEntryNotFound(String),

    #[error("unknown decoding error")]
    Unknown,

    #[error("Parsing failed: {0}")]
    ParseError(String),

    #[error("Unknown image data format: `{0}`!\nPlease report this error at https://github.com/arma-tools/arma-file-formats-rs/issues")]
    UnknownImageDataFormat(String),

    #[error("Unknown image data type: `{0}`!\nPlease report this error at https://github.com/arma-tools/arma-file-formats-rs/issues")]
    UnknownImageDataType(String),
}
