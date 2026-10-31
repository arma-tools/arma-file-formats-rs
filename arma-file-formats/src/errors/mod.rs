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

    #[error("Lod not found")]
    LodNotFound,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum XobError {
    #[error("Unknown form type: `{0}`")]
    UnknownFormType(String),

    #[error("Chunk `{0}` missing")]
    ChunkMissing(&'static str),

    #[error("LOD {0} data lies outside of the LODS chunk")]
    LodDataOutOfBounds(usize),

    #[error("LOD {lod} decompressed to {actual} bytes, expected {expected}")]
    LodSizeMismatch {
        lod: usize,
        expected: usize,
        actual: usize,
    },

    #[error("LZ4 decompression of LOD {0} failed")]
    Decompression(usize),

    #[error("Unknown collider shape: `{0}`")]
    UnknownColliderShape(u8),
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum PakError {
    #[error("Unknown form type: `{0}`")]
    UnknownFormType(String),

    #[error("Chunk `{0}` missing")]
    ChunkMissing(&'static str),

    #[error("Invalid HEAD chunk")]
    InvalidHead,

    #[error("Unsupported pak version: `{0:#x}`")]
    UnsupportedVersion(u32),

    #[error("FILE chunk does not match the checksum in the HEAD chunk")]
    ChecksumMismatch,

    #[error("Unknown compression: `{0}`")]
    UnknownCompression(u8),

    #[error("`{path}`: expected {expected} bytes, read {actual}")]
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },

    #[error("Invalid path: `{0}`")]
    InvalidPath(String),

    #[error("Duplicate path: `{0}`")]
    DuplicatePath(String),

    #[error("Pak exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum EddsError {
    #[error("LZ4 decompression of mipmap {0} failed")]
    Decompression(usize),

    #[error("Mipmap {mipmap} has {actual} bytes, expected at least {expected}")]
    MipmapTooShort {
        mipmap: usize,
        expected: usize,
        actual: usize,
    },

    #[error("Mipmap index {0} out of range")]
    MipmapOutOfRange(usize),
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum TextError {
    #[error("Line {line}: {message}")]
    Parse { line: usize, message: String },
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum EmatError {
    #[error("Parent material `{0}` not found")]
    ParentNotFound(String),

    #[error("Material inheritance loops at `{0}`")]
    InheritanceCycle(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum TerrError {
    #[error("Unknown form type: `{0}`")]
    UnknownFormType(String),

    #[error("Chunk `{0}` missing")]
    ChunkMissing(&'static str),

    #[error("Chunk `{0}` is truncated, too long or malformed")]
    InvalidChunk(&'static str),

    #[error("Terr exceeds 4 GiB")]
    TooLarge,

    #[error("Chunk `{0}` can't hold the given data")]
    Unwritable(&'static str),
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum EntError {
    #[error("Not a binary world, magic `{0:02X?}` instead of `EBIN`")]
    UnknownMagic(Vec<u8>),

    #[error("Unexpected end of data at offset {0}")]
    Truncated(usize),

    #[error("Unknown token `{tag:#04x}` at offset {offset}")]
    UnknownToken { tag: u8, offset: usize },

    #[error("Offset {offset}: {message}")]
    Malformed {
        offset: usize,
        message: &'static str,
    },

    #[error("{0} exceeds the limits of the format")]
    TooLarge(&'static str),

    #[error("Can't write: {0}")]
    Unwritable(&'static str),
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum TopoError {
    #[error("Not a topo, magic `{0:02X?}` instead of `TOPO`")]
    UnknownMagic(Vec<u8>),

    #[error("Unexpected end of data at offset {0}")]
    Truncated(usize),

    #[error("Invalid chunk header at offset {0}")]
    InvalidHeader(usize),

    #[error("Chunk `{0}` missing")]
    ChunkMissing(&'static str),

    #[error("Unsupported version {version} of chunk `{chunk}`")]
    UnsupportedVersion { chunk: &'static str, version: u32 },

    #[error("Chunk `{0}` is truncated, too long or malformed")]
    InvalidChunk(&'static str),

    #[error("Chunk `{0}` can't hold the given data")]
    Unwritable(&'static str),

    #[error("Topo exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum SmdError {
    #[error("Unknown form type: `{0}`")]
    UnknownFormType(String),

    #[error("Chunk `{0}` missing")]
    ChunkMissing(&'static str),

    #[error("Unsupported version {0}")]
    UnsupportedVersion(u32),

    #[error("Chunk `{0}` is truncated, too long or malformed")]
    InvalidChunk(&'static str),

    #[error("Chunk `{0}` can't hold the given data")]
    Unwritable(&'static str),

    #[error("Smd exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum SmapError {
    #[error("Truncated or malformed sound map grid")]
    InvalidGrid,

    #[error("Unknown form type: `{0}`")]
    UnknownFormType(String),

    #[error("Chunk `{0}` missing")]
    ChunkMissing(&'static str),

    #[error("Unsupported version {0}")]
    UnsupportedVersion(u32),

    #[error("Chunk `{0}` is truncated, too long or malformed")]
    InvalidChunk(&'static str),

    #[error("LZ4 decompression of block {0}, {1} failed")]
    Decompression(u16, u16),

    #[error("`{0}` can't hold the given data")]
    Unwritable(&'static str),

    #[error("Smap exceeds 4 GiB")]
    TooLarge,
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

    #[error("XOB Error: {0}")]
    XobError(#[from] XobError),

    #[error("PAK Error: {0}")]
    PakError(#[from] PakError),

    #[error("TERR Error: {0}")]
    TerrError(#[from] TerrError),

    #[error("ENT Error: {0}")]
    EntError(#[from] EntError),

    #[error("TOPO Error: {0}")]
    TopoError(#[from] TopoError),

    #[error("SMD Error: {0}")]
    SmdError(#[from] SmdError),

    #[error("SMAP Error: {0}")]
    SmapError(#[from] SmapError),

    #[error("EDDS Error: {0}")]
    EddsError(#[from] EddsError),

    #[error("Text resource Error: {0}")]
    TextError(#[from] TextError),

    #[error("EMAT Error: {0}")]
    EmatError(#[from] EmatError),

    #[error("Invalid file")]
    InvalidFileError,

    #[error("PBO Entry {0} not found")]
    PboEntryNotFound(String),

    #[error("unknown decoding error")]
    Unknown,

    #[error("Parsing failed: {0}")]
    ParseError(String),

    #[error(
        "Unknown image data format: `{0}`!\nPlease report this error at https://github.com/arma-tools/arma-file-formats-rs/issues"
    )]
    UnknownImageDataFormat(String),

    #[error(
        "Unknown image data type: `{0}`!\nPlease report this error at https://github.com/arma-tools/arma-file-formats-rs/issues"
    )]
    UnknownImageDataType(String),
}
