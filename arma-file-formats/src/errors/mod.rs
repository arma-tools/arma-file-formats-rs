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

    #[error("PNG image, not a PAA")]
    Png,

    #[error("Unknown PAA type `{0:#06x}`")]
    UnknownType(u16),

    #[error("PAA type `{0:#06x}` can't be decoded")]
    UnsupportedDecode(u16),

    #[error("PAA type `{0:#06x}` can't be encoded")]
    UnsupportedEncode(u16),

    /// The file is cut off, like the decoys of obfuscated PBOs: copies of real textures that
    /// end before their mipmaps.
    #[error("Mipmap {index} ends at byte {end}, past the end of the file ({len} bytes)")]
    MipmapPastEnd { index: usize, end: u64, len: u64 },
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

    /// The file is cut off, like the decoys of obfuscated PBOs: copies of real models that end
    /// before their LODs.
    #[error("LOD {index} starts at byte {start}, past the end of the file ({len} bytes)")]
    LodPastEnd { index: usize, start: u64, len: u64 },
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

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum RdbError {
    #[error("Unknown form type: `{0}`")]
    UnknownFormType(String),

    #[error("Unsupported version {0}")]
    UnsupportedVersion(u32),

    #[error("Header says the file has {header} bytes, it has {actual}")]
    SizeMismatch { header: u64, actual: u64 },

    #[error("Truncated or malformed resource database")]
    Truncated,

    #[error("Data after the last entry")]
    TrailingData,

    #[error("String is not NUL terminated UTF-8")]
    InvalidString,

    #[error("Flags and `.meta` of entry `{0}` don't match")]
    MetaFlagMismatch(String),

    #[error("Rdb exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum FntError {
    #[error("Unknown form type: `{0}`, only FNT2 to FNT5 are known")]
    UnknownFormType(String),

    #[error("Truncated font")]
    Truncated,

    #[error("Unknown chunk `{0}`")]
    UnknownChunk(String),

    #[error("Chunk `{0}` appears twice")]
    DuplicateChunk(String),

    #[error("Chunk `{0}` missing")]
    ChunkMissing(&'static str),

    #[error("Chunk `{0}` is truncated, too long or malformed")]
    InvalidChunk(&'static str),

    #[error("Unknown font type {0}")]
    UnknownFontType(u8),

    #[error("`{0}` can't hold the given data")]
    Unwritable(&'static str),

    #[error("Fnt exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum SndError {
    #[error("Not a RIFF file")]
    NotRiff,

    #[error("Unknown form type: `{0}`, only OPUS is known")]
    UnknownFormType(String),

    #[error("Truncated sound")]
    Truncated,

    #[error("Unknown chunk `{0}`")]
    UnknownChunk(String),

    #[error("Chunk `{0}` appears twice")]
    DuplicateChunk(&'static str),

    #[error("Chunk `{0}` missing")]
    ChunkMissing(&'static str),

    #[error("Chunk `{tag}` has the unknown size {size}")]
    ChunkSize { tag: &'static str, size: u32 },

    #[error("Packet {0} exceeds 65535 bytes")]
    PacketTooLarge(usize),

    #[error("Packet {0} is empty")]
    InvalidPacket(usize),

    #[error("Ogg Opus without channel mapping needs 1 or 2 channels, not {0}")]
    UnsupportedChannels(u16),

    #[error("Pre-skip {0} exceeds 65535 samples")]
    PreSkipTooLarge(u32),

    #[error("Snd exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum WssError {
    #[error("Unknown signature: `{0}`, expected WSS0")]
    UnknownSignature(String),

    #[error("Truncated sound")]
    Truncated,

    #[error("Unknown compression: `{0}`, known are 0, 4 and 8")]
    UnknownCompression(u32),

    #[error("Only 16 bit PCM decodes to 16 bit samples, not {0} bit")]
    UnsupportedBits(u16),

    #[error("Wav exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum TexHeadersError {
    #[error("Unknown signature: `{0}`, expected 0DHT")]
    UnknownSignature(String),

    #[error("Unsupported version: `{0}`, only 1 is known")]
    UnsupportedVersion(u32),

    #[error("Truncated texture headers")]
    Truncated,

    #[error("{0} bytes after the last texture")]
    TrailingData(usize),

    #[error("Mipmap counts {0} and {1} of a texture differ")]
    MipmapCountMismatch(u32, u32),

    #[error("Unknown pixel format: `{0}`")]
    UnknownPixelFormat(u32),

    #[error("Byte `{0}` is not a bool")]
    InvalidBool(u8),

    #[error("String is not UTF-8 without NULs: {0:?}")]
    InvalidString(Vec<u8>),

    #[error("Path `{0}` contains a NUL")]
    InvalidPath(String),

    #[error("Unknown paa type: `{0:#06x}`")]
    UnknownPaaType(u16),

    #[error("Tag `{0}` has {1} bytes, expected 4")]
    InvalidTag(String, usize),

    #[error("Texture exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum FxyError {
    #[error("Unknown signature: `{0}`, expected BIFo")]
    UnknownSignature(String),

    #[error("Unsupported version: `{0:#x}`, known are 0x101 and 0x102")]
    UnsupportedVersion(u32),

    #[error("Truncated font")]
    Truncated,

    #[error("Character `{0:#x}` is below 0x20 or above 0x1001f, version 1 can't store it")]
    InvalidCode(u32),

    #[error("Page exceeds 4 GiB entries")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum LipError {
    #[error("Lip sync is not UTF-8")]
    InvalidText,

    #[error("Missing the `frame = ...` line")]
    MissingFrame,

    #[error("Line {0} is not `time, value`: `{1}`")]
    InvalidLine(usize, String),

    #[error("Line {0} follows the `-1` that ends the lip sync")]
    AfterEnd(usize),
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum SqfcError {
    #[error("Unsupported version: `{0}`, known are 1 and 2")]
    UnsupportedVersion(u32),

    #[error("Unknown block type: `{0}`")]
    UnknownBlock(u8),

    #[error("Block type `{0}` is not supported")]
    UnsupportedBlock(u8),

    #[error("Block type `{0}` appears twice or out of order")]
    RepeatedBlock(u8),

    #[error("Block type `{0}` is missing")]
    MissingBlock(u8),

    #[error("Unknown compression: `{0}`, known is 2 (LZO)")]
    UnknownCompression(u8),

    #[error("LZO: {0}")]
    Lzo(String),

    #[error("Truncated script")]
    Truncated,

    #[error("{0} bytes after the end of a block")]
    TrailingData(usize),

    #[error("Unknown constant type: `{0}`")]
    UnknownConstant(u8),

    #[error("Unknown instruction: `{0}`")]
    UnknownInstruction(u8),

    #[error("Boolean is `{0}`, expected 0 or 1")]
    InvalidBoolean(u8),

    #[error("Name index `{0}` is past the name table")]
    InvalidName(u16),

    #[error("String is not UTF-8")]
    InvalidString,

    #[error("Hash map needs version 2 and an even number of items")]
    InvalidHashMap,

    #[error("Array flags `{0}`, version 1 can't store them")]
    InvalidFlags(u8),

    #[error("Code source constant has the top bit set, or its range is 2 GiB or more")]
    InvalidSource,

    #[error("String of {0} bytes, at most 16 MiB - 1 fit")]
    StringTooLong(usize),

    #[error("Array of {0} values, version 1 makes at most 65535")]
    ArrayTooLarge(u32),

    #[error("More than 65535 constants")]
    TooManyConstants,

    #[error("More than 65535 names")]
    TooManyNames,

    #[error("More than 65535 files")]
    TooManyFiles,

    #[error("More than 4 Gi instructions")]
    TooManyInstructions,

    #[error("Block exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum MlodError {
    #[error("Unknown signature: `{0}`, expected MLOD")]
    UnknownSignature(String),

    #[error("Unsupported version: `{0:#x}`, known is 0x101")]
    UnsupportedVersion(u32),

    #[error("Unknown LOD signature: `{0}`, expected P3DM")]
    UnknownLodSignature(String),

    #[error("Unsupported P3DM version: `{major:#x}.{minor:#x}`, known is 0x1c.0x100")]
    UnsupportedLodVersion { major: u32, minor: u32 },

    #[error("Truncated model")]
    Truncated,

    #[error("{0} bytes after the last LOD")]
    TrailingData(usize),

    #[error("Face has {0} sides, expected 3 or 4")]
    InvalidSides(u32),

    #[error("TAGG missing after the faces")]
    TaggsMissing,

    #[error("Tagg `{name}` has {size} bytes, expected {expected}")]
    InvalidTaggSize {
        name: String,
        size: usize,
        expected: usize,
    },

    #[error("Tagg `{name}` has {len} entries, the LOD needs {expected}")]
    InvalidTaggLength {
        name: String,
        len: usize,
        expected: usize,
    },

    #[error("Selection `{name}` has {points} points and {faces} faces, not those of the LOD")]
    InvalidSelection {
        name: String,
        points: usize,
        faces: usize,
    },

    #[error("Selection name `{0}` is empty or starts with `#`")]
    InvalidSelectionName(String),

    #[error("Property `{0}` has 64 bytes or more, or a NUL")]
    InvalidProperty(String),

    #[error("`{0}` has a NUL")]
    InvalidName(String),

    #[error("Tagg `#EndOfFile#` can't be written in the middle of a LOD")]
    EndOfFileTagg,

    #[error("String is not UTF-8: {0:02x?}")]
    InvalidString(Vec<u8>),

    #[error("Model exceeds 4 GiB entries")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum RtmError {
    #[error("Unknown signature: `{0}`")]
    UnknownSignature(String),

    #[error("Unsupported BMTR version: `{0}`")]
    UnsupportedVersion(u32),

    #[error("Truncated animation")]
    Truncated,

    #[error("Header says the animation has {header} bones, it names {names}")]
    BoneCountMismatch { header: usize, names: usize },

    #[error("Header says the animation has {header} frames, it has {times} frame times")]
    FrameCountMismatch { header: usize, times: usize },

    #[error("Frame {frame} has {actual} transforms, expected {expected}")]
    TransformCountMismatch {
        frame: usize,
        expected: usize,
        actual: usize,
    },

    #[error("Unknown array compression flag: `{0}`")]
    UnknownCompression(u8),

    #[error("LZO decompression failed: {0}")]
    Lzo(String),

    #[error("Bone name `{0}` is not up to 31 bytes without NULs")]
    InvalidBoneName(String),

    #[error("String is not UTF-8 without NULs: {0:?}")]
    InvalidString(Vec<u8>),

    #[error("BMTR version 3 has no metadata and keystones")]
    MetadataUnsupported,

    #[error("RTM_0100 has only the z of the step")]
    StepUnsupported,

    #[error("`{0}` is longer than 255 bytes")]
    StringTooLong(String),

    #[error("Animation exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum StarsError {
    #[error("Unknown form type: `{0}`")]
    UnknownFormType(String),

    #[error("Truncated star catalog")]
    Truncated,

    #[error("Header says the catalog has {header} stars, it has {items}")]
    CountMismatch { header: u32, items: usize },

    #[error("Star {0} is truncated, too long or malformed")]
    InvalidItem(usize),

    #[error("Name of star {0} is not NUL terminated UTF-8 without NULs")]
    InvalidName(usize),

    #[error("Star catalog exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum AnmError {
    #[error("Unknown form type: `{0}`")]
    UnknownFormType(String),

    #[error("Unsupported animation set `{0}`, only SET6 and SET7 are known")]
    UnsupportedSet(String),

    #[error("Chunk `{0}` missing")]
    ChunkMissing(&'static str),

    #[error("Chunk `{0}` is truncated, too long or malformed")]
    InvalidChunk(&'static str),

    #[error("Chunk `{0}` can't hold the given data")]
    Unwritable(&'static str),

    #[error("Anm exceeds 4 GiB")]
    TooLarge,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum NavmeshError {
    #[error("Unknown navmesh magic {0:#010x}")]
    UnknownMagic(u32),

    #[error("Unsupported IFF version {0}")]
    UnsupportedVersion(u32),

    #[error("Unknown form type: `{0}`")]
    UnknownFormType(String),

    #[error("Truncated or malformed navmesh")]
    Truncated,

    #[error("Chunk `{0}` missing")]
    ChunkMissing(&'static str),

    #[error("Expected chunk `{expected}`, found `{found}`")]
    UnexpectedChunk {
        expected: &'static str,
        found: String,
    },

    #[error("Chunk `{0}` is truncated, too long or malformed")]
    InvalidChunk(&'static str),

    #[error("Data after the last chunk or tile")]
    TrailingData,

    #[error("Path of {0} bytes is too long")]
    PathTooLong(usize),

    #[error("Path is not UTF-8")]
    InvalidPath,

    #[error("Unsupported tile version {0}")]
    UnsupportedTileVersion(i32),

    #[error("The file has stock Detour tiles of an IFF version 2 navmesh, read it as `NtileV2`")]
    DetourTiles,

    #[error("Unsupported tile meta data version {0}")]
    UnsupportedMetaVersion(u32),

    #[error("Malformed tile: {0}")]
    InvalidTile(&'static str),

    #[error("`{0}` can't hold the given data")]
    Unwritable(&'static str),

    #[error("Navmesh file exceeds the size limits")]
    TooLarge,
}

#[derive(Debug, Error)]
pub enum RapError {
    #[error("unknown config entry type {0}")]
    UnknownEntryType(u8),

    #[error("unknown config value type {0}")]
    UnknownValueType(u8),

    #[error("unknown flags {0} of an array extension (`+=`)")]
    UnknownArrayFlags(u32),

    #[error("classes nested deeper than {0} levels")]
    TooDeep(usize),
}

/// A config preprocessor error with the file and line it happened at.
#[derive(Debug, Error)]
#[error("{}:{line}: {kind}", file.as_deref().unwrap_or("<input>"))]
pub struct PreprocessError {
    /// Path of the file, `None` for a source passed in without a path
    pub file: Option<String>,
    /// 1-based, 0 for errors outside of a source (`Preprocessor::define`)
    pub line: usize,
    pub kind: PreprocessErrorKind,
}

#[derive(Debug, Error)]
pub enum PreprocessErrorKind {
    #[error("unknown directive `#{0}`")]
    UnknownDirective(String),

    #[error("`#{0}` needs a macro name")]
    MissingMacroName(String),

    #[error("invalid macro parameter list")]
    InvalidParams,

    #[error("`{name}` takes {}{expected} argument(s), {found} given", if *variadic { "at least " } else { "" })]
    ArgCount {
        name: String,
        expected: usize,
        variadic: bool,
        found: usize,
    },

    #[error("missing `)` in the arguments of `{0}`")]
    UnterminatedCall(String),

    #[error("`#{0}` without a matching #if")]
    Unmatched(String),

    #[error("`#{0}` after #else")]
    AfterElse(String),

    #[error("#if without a matching #endif")]
    UnterminatedIf,

    #[error("invalid #if condition `{0}`")]
    InvalidCondition(String),

    #[error("invalid include path `{0}`, it must be in \"\" or <>")]
    InvalidInclude(String),

    #[error("include file `{0}` not found")]
    IncludeNotFound(String),

    #[error("can't include `{0}` without an include resolver")]
    NoIncludeResolver(String),

    #[error("reading include file `{path}` failed: {source}")]
    IncludeIo {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("includes nested deeper than {0} levels")]
    IncludeDepth(usize),

    #[error("invalid __VA_SELECT__ index `{0}`")]
    InvalidVaSelect(String),

    #[error("__VA_APPLY__ needs a macro name, got `{0}`")]
    InvalidVaApply(String),
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

    #[error("RDB Error: {0}")]
    RdbError(#[from] RdbError),

    #[error("ANM Error: {0}")]
    AnmError(#[from] AnmError),

    #[error("FNT Error: {0}")]
    FntError(#[from] FntError),

    #[error("SND Error: {0}")]
    SndError(#[from] SndError),

    #[error("Navmesh Error: {0}")]
    NavmeshError(#[from] NavmeshError),

    #[error("RTM Error: {0}")]
    RtmError(#[from] RtmError),

    #[error("TexHeaders Error: {0}")]
    TexHeadersError(#[from] TexHeadersError),

    #[error("FXY Error: {0}")]
    FxyError(#[from] FxyError),

    #[error("MLOD Error: {0}")]
    MlodError(#[from] MlodError),

    #[error("LIP Error: {0}")]
    LipError(#[from] LipError),

    #[error("SQFC Error: {0}")]
    SqfcError(#[from] SqfcError),

    #[error("WSS Error: {0}")]
    WssError(#[from] WssError),

    #[error("STARS Error: {0}")]
    StarsError(#[from] StarsError),

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

    #[error("Preprocessing failed: {0}")]
    PreprocessError(#[from] PreprocessError),

    #[error("Config error: {0}")]
    RapError(#[from] RapError),

    #[error(
        "Unknown image data format: `{0}`!\nPlease report this error at https://github.com/arma-tools/arma-file-formats-rs/issues"
    )]
    UnknownImageDataFormat(String),

    #[error(
        "Unknown image data type: `{0}`!\nPlease report this error at https://github.com/arma-tools/arma-file-formats-rs/issues"
    )]
    UnknownImageDataType(String),
}
