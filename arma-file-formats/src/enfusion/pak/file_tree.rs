use binrw::binrw;

use crate::core::iff::{ChunkHeader, FormHeader};

pub(super) const FORM_TYPE: &[u8; 4] = b"PAC1";
pub(super) const HEAD_SIZE: usize = 28;
pub(super) const HEAD_VERSION: u32 = 0x0001_0003;
const CHECKSUM_KEY: &[u8; 16] = b"BIStudio19092012";

/// Offset of the first file in a pak: `FORM` header, `HEAD` chunk and `DATA` chunk header.
pub(super) const DATA_OFFSET: u32 =
    FormHeader::SIZE + ChunkHeader::SIZE + HEAD_SIZE as u32 + ChunkHeader::SIZE;

pub(super) const COMPRESSION_STORED: u8 = 0;
pub(super) const COMPRESSION_ZLIB: u8 = 1;

/// Node of the `FILE` chunk.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub(super) enum Node {
    #[brw(magic = 0u8)]
    Dir {
        name: NodeName,
        #[br(temp)]
        #[bw(calc = children.len() as u32)]
        count: u32,
        #[br(count = count)]
        children: Vec<Self>,
    },
    #[brw(magic = 1u8)]
    File { name: NodeName, record: FileRecord },
}

#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub(super) struct NodeName {
    #[br(temp)]
    #[bw(calc = bytes.len() as u8)]
    len: u8,
    #[br(count = len)]
    pub bytes: Vec<u8>,
}

#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub(super) struct FileRecord {
    pub offset: u32,
    pub compressed_size: u32,
    #[brw(pad_after = 6)]
    pub size: u32,
    pub compression: u8,
    pub compression_level: u8,
    pub timestamp: u32,
}

/// Contents of the `HEAD` chunk for a `FILE` chunk with the given CRC32.
pub(super) fn encode_head(file_crc: u32) -> [u8; HEAD_SIZE] {
    let mut head = [0; HEAD_SIZE];
    head[0..4].copy_from_slice(&HEAD_VERSION.to_le_bytes());
    head[4..8].copy_from_slice(&file_crc.to_le_bytes());
    let hex = format!("{file_crc:08x}{file_crc:08x}");
    for ((out, hex), key) in head[8..24].iter_mut().zip(hex.bytes()).zip(CHECKSUM_KEY) {
        *out = hex ^ key;
    }
    head
}

pub(super) fn crc32(data: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();
    crc.update(data);
    crc.sum()
}
