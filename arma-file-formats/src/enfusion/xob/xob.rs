use std::io::{Cursor, Read, Seek, SeekFrom};

use binrw::BinRead;
use lzzzz::lz4;

use crate::{
    core::read::ReadExtTrait,
    enfusion::xob::{
        XobCollider, XobHead, XobLodHeader, XobMesh, XobOccupancyGrid, XobSectionHeader,
    },
    errors::{AffError, XobError},
};

const XOB_FORM_TYPE: &[u8; 4] = b"XOB9";
const LZ4_BLOCK_SIZE: usize = 0x10000;
const LZ4_LAST_BLOCK_FLAG: u32 = 0x8000_0000;

/// Enfusion model (`.xob`).
#[derive(Debug, Clone)]
pub struct Xob {
    pub head: XobHead,
    /// Decoded LODs, in the order of [`XobHead::lods`] (most detailed first).
    pub lods: Vec<XobLod>,
    /// Contents of the `COLL` chunk.
    pub colliders: Vec<XobCollider>,
    /// `VOLM` chunk, absent for flat models.
    pub occupancy: Option<XobOccupancyGrid>,
}

#[derive(Debug, Clone)]
pub struct XobLod {
    /// One mesh per entry in [`XobLodHeader::sections`].
    pub sections: Vec<XobMesh>,
}

struct Chunk {
    tag: [u8; 4],
    offset: u64,
    data: Vec<u8>,
}

impl Xob {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let chunks = read_chunks(input)?;
        let find = |tag: &[u8; 4]| chunks.iter().find(|c| &c.tag == tag);

        let head_chunk = find(b"HEAD").ok_or(XobError::ChunkMissing("HEAD"))?;
        let head = XobHead::read(&mut Cursor::new(&head_chunk.data))?;

        let lods = match find(b"LODS") {
            Some(lods_chunk) => head
                .lods
                .iter()
                .enumerate()
                .map(|(index, lod)| read_lod(lods_chunk, index, lod))
                .collect::<Result<_, _>>()?,
            None if head.lods.is_empty() => Vec::new(),
            None => return Err(XobError::ChunkMissing("LODS").into()),
        };

        let occupancy = find(b"VOLM")
            .map(|c| XobOccupancyGrid::read(&mut Cursor::new(&c.data)))
            .transpose()?;

        let colliders = match find(b"COLL") {
            Some(chunk) => XobCollider::read_all(&chunk.data, &head.strings)?,
            None => Vec::new(),
        };

        Ok(Self {
            head,
            lods,
            colliders,
            occupancy,
        })
    }
}

fn read_chunks<I: Seek + Read>(input: &mut I) -> Result<Vec<Chunk>, AffError> {
    if input.read_string_lossy(4)? != "FORM" {
        return Err(AffError::InvalidFileError);
    }
    let form_end = u64::from(input.read_u32_be()?) + 8;

    let mut form_type = [0; 4];
    input.read_exact(&mut form_type)?;
    if &form_type != XOB_FORM_TYPE {
        return Err(
            XobError::UnknownFormType(String::from_utf8_lossy(&form_type).into_owned()).into(),
        );
    }

    let mut chunks = Vec::new();
    while input.stream_position()? < form_end {
        let mut tag = [0; 4];
        input.read_exact(&mut tag)?;
        let size = input.read_u32_be()? as usize;
        let offset = input.stream_position()?;
        let data = input.read_bytes(size)?;
        chunks.push(Chunk { tag, offset, data });
    }
    input.seek(SeekFrom::Start(form_end))?;

    Ok(chunks)
}

fn read_lod(lods: &Chunk, index: usize, header: &XobLodHeader) -> Result<XobLod, AffError> {
    let start = u64::from(header.data_offset)
        .checked_sub(lods.offset)
        .ok_or(XobError::LodDataOutOfBounds(index))? as usize;
    let compressed = lods
        .data
        .get(start..start + header.compressed_size as usize)
        .ok_or(XobError::LodDataOutOfBounds(index))?;

    let data = decompress_lz4_chain(compressed, header.size as usize)
        .ok_or(XobError::Decompression(index))?;

    let expected = header
        .sections
        .iter()
        .map(XobSectionHeader::data_size)
        .sum::<usize>();
    if data.len() != header.size as usize || data.len() != expected {
        return Err(XobError::LodSizeMismatch {
            lod: index,
            expected,
            actual: data.len(),
        }
        .into());
    }

    let mut cursor = Cursor::new(data.as_slice());
    let sections = header
        .sections
        .iter()
        .map(|section| XobMesh::read(&mut cursor, section))
        .collect::<Result<_, _>>()?;

    Ok(XobLod { sections })
}

/// Each block starts with a `u32` header: the compressed size in the lower 24 bits and a flag
/// for the last block. Blocks decompress to 64 KiB and use the previous output as dictionary.
fn decompress_lz4_chain(mut src: &[u8], size: usize) -> Option<Vec<u8>> {
    let mut decompressor = lz4::Decompressor::new().ok()?;
    let mut out = Vec::with_capacity(size);

    loop {
        let block_header = u32::from_le_bytes(src.get(..4)?.try_into().ok()?);
        let block_size = (block_header & 0x00FF_FFFF) as usize;
        let block = src.get(4..4 + block_size)?;
        src = &src[4 + block_size..];

        let is_last = block_header & LZ4_LAST_BLOCK_FLAG != 0;
        let original_size = if is_last {
            size.checked_sub(out.len())?
        } else {
            LZ4_BLOCK_SIZE
        };
        out.extend_from_slice(decompressor.next(block, original_size).ok()?);

        if is_last {
            return Some(out);
        }
    }
}
