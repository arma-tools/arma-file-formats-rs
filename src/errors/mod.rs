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

    #[error("Invalid state")]
    InvalidState,

    #[error("unknown decoding error")]
    Unknown,
}

#[derive(Error, Debug)]
pub enum RvffError {
    #[error("IO failed")]
    RvffIOError(#[from] io::Error),

    #[error("Deku failed")]
    RvffDekuError(#[from] deku::DekuError),

    #[error("Invalid file")]
    InvalidFileError,

    #[error("unknown decoding error")]
    Unknown,
}
