use std::io;

use lzokay_rust_native::util::LzokayError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PaaError {
    #[error("No mipmaps were set")]
    NoMipmapError,

    #[error("Invalid mipmap at index `{0}`")]
    InvalidMipmapError(usize),

    #[error("IO failed")]
    PaaIOError(#[from] io::Error),

    #[error("Paa lzo failed")]
    PaaLzoErr(#[from] LzokayError),

    #[error("LZSS Error")]
    RvffLzssError(#[from] RvffLzssError),

    #[error("Invalid state")]
    InvalidState,

    #[error("unknown decoding error")]
    Unknown,
}

#[derive(Error, Debug)]
pub enum RvffLzssError {
    #[error("LZSS Checksum Missmatch")]
    ChecksumMissmatch,

    #[error("LZSS Overflow")]
    Overflow,
}

#[derive(Error, Debug)]
pub enum RvffOdolError {
    #[error("Signature Missing")]
    SignatureMissing,

    #[error("Unknown Version: `{0}`")]
    UnknownVersion(u32),

    #[error("Unsupported Version: `{0}`")]
    UnsupportedVersion(u32),
}

#[derive(Error, Debug)]
pub enum RvffError {
    #[error("IO failed")]
    RvffIOError(#[from] io::Error),

    #[error("Deku failed")]
    RvffDekuError(#[from] deku::DekuError),

    #[error("LZSS Error")]
    RvffLzssError(#[from] RvffLzssError),

    #[error("ODOL Error")]
    RvffOdolError(#[from] RvffOdolError),

    #[error("Invalid file")]
    InvalidFileError,

    #[error("unknown decoding error")]
    Unknown,
}

#[derive(Error, Debug)]
pub enum RvffConfigErrorKind {
    #[error("IO failed")]
    RvffIOError(#[from] io::Error),

    #[error("Parsing failed: {0}")]
    RvffPestError(String),

    #[error("Invalid file")]
    InvalidFileError,

    #[error("unknown decoding error")]
    Unknown,
}

#[derive(Error, Debug)]
#[error(transparent)]
pub struct RvffConfigError(Box<RvffConfigErrorKind>);

impl<E> From<E> for RvffConfigError
where
    RvffConfigErrorKind: From<E>,
{
    fn from(err: E) -> Self {
        RvffConfigError(Box::new(RvffConfigErrorKind::from(err)))
    }
}
