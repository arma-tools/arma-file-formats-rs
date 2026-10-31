use std::io::{Read, Seek, SeekFrom};

use binrw::BinRead;

use crate::{
    core::{read::ReadExtTrait, types::PixelType},
    enfusion::edds::{DdsHeader, EddsFormat},
    errors::{AffError, EddsError},
};

const LZ4_BLOCK_SIZE: usize = 0x10000;

/// Enfusion texture (`.edds`) with every mipmap decoded to pixels, see [`Edds::pixel_type`].
#[derive(Debug, Clone)]
pub struct Edds {
    /// Layout of [`Mipmap::data`].
    pub pixel_type: PixelType,
    pub header: DdsHeader,
    /// Format the texture is stored in.
    pub format: EddsFormat,
    /// Smallest first.
    pub mipmaps: Vec<Mipmap>,
}

#[derive(Debug, Clone)]
pub enum MipmapType {
    COPY,
    LZ4,
}

#[derive(Debug, Clone)]
pub struct Mipmap {
    pub width: usize,
    pub height: usize,
    pub data_type: MipmapType,
    pub compressed_data_size: u32,
    pub data: Vec<u8>,
}

/// Enfusion texture (`.edds`) with the mipmaps in their stored [`EddsFormat`], e.g. still block
/// compressed. Useful to upload them to a GPU as they are, or to decode only some of them.
#[derive(Debug, Clone)]
pub struct RawEdds {
    pub header: DdsHeader,
    pub format: EddsFormat,
    /// Smallest first, LZ4 compression is already undone.
    pub mipmaps: Vec<Mipmap>,
}

impl Edds {
    /// Reads the texture and decodes every mipmap.
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        RawEdds::from(input)?.decode()
    }
}

impl RawEdds {
    /// Reads the header and every mipmap without decoding the pixels.
    ///
    /// An EDDS is a DDS header (optionally with the DX10 extension), followed by a table with
    /// one entry per mipmap, smallest first: the storage (`COPY` or `LZ4 `) and the stored size.
    /// The mipmap data follows in the same order. `LZ4 ` data starts with the `u32` unpacked
    /// size, followed by blocks with a `u24` compressed size and a `u8` last block flag. Blocks
    /// unpack to 64 KiB (except the last one) and use the previous output as dictionary.
    ///
    /// A few `.edds` files are plain DDS files without the table, their mipmaps are stored
    /// uncompressed, largest first. They are read as well.
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let header = DdsHeader::read(input)?;
        let format = EddsFormat::from_header(&header)?;
        let levels = header.mipmap_count.max(1);

        let data_start = input.stream_position()?;
        let first_tag = input.read_bytes(4)?;
        input.seek(SeekFrom::Start(data_start))?;
        if !matches!(&first_tag[..], b"COPY" | b"LZ4 ") {
            return read_plain_dds(input, header, format, levels);
        }

        let mut mipmaps = Vec::with_capacity(levels as usize);
        for level in (0..levels).rev() {
            let data_type = match &input.read_bytes(4)?[..] {
                b"COPY" => MipmapType::COPY,
                b"LZ4 " => MipmapType::LZ4,
                unknown => {
                    return Err(AffError::UnknownImageDataType(
                        String::from_utf8_lossy(unknown).into_owned(),
                    ));
                }
            };
            mipmaps.push(Mipmap {
                width: mip_dimension(header.width, level),
                height: mip_dimension(header.height, level),
                data_type,
                compressed_data_size: input.read_u32()?,
                data: Vec::new(),
            });
        }

        for (index, mipmap) in mipmaps.iter_mut().enumerate() {
            let stored = input.read_bytes(mipmap.compressed_data_size as usize)?;
            mipmap.data = match mipmap.data_type {
                MipmapType::COPY => stored,
                MipmapType::LZ4 => {
                    decompress_lz4_chain(&stored).ok_or(EddsError::Decompression(index))?
                }
            };
            let expected = format.data_size(mipmap.width, mipmap.height);
            if mipmap.data.len() < expected {
                return Err(EddsError::MipmapTooShort {
                    mipmap: index,
                    expected,
                    actual: mipmap.data.len(),
                }
                .into());
            }
        }

        Ok(Self {
            header,
            format,
            mipmaps,
        })
    }

    /// Decodes the mipmap at `index` (smallest first), see [`EddsFormat::decode`].
    pub fn decode_mipmap(&self, index: usize) -> Result<Vec<u8>, AffError> {
        let mipmap = self
            .mipmaps
            .get(index)
            .ok_or(EddsError::MipmapOutOfRange(index))?;
        self.format
            .decode(&mipmap.data, mipmap.width, mipmap.height)
    }

    /// Decodes every mipmap.
    pub fn decode(mut self) -> Result<Edds, AffError> {
        for mipmap in &mut self.mipmaps {
            mipmap.data = self
                .format
                .decode(&mipmap.data, mipmap.width, mipmap.height)?;
        }
        Ok(Edds {
            pixel_type: self.format.decoded_pixel_type(),
            header: self.header,
            format: self.format,
            mipmaps: self.mipmaps,
        })
    }
}

/// Reads the mipmaps of a DDS without the EDDS table: uncompressed, largest first.
fn read_plain_dds<I: Read + Seek>(
    input: &mut I,
    header: DdsHeader,
    format: EddsFormat,
    levels: u32,
) -> Result<RawEdds, AffError> {
    let mut mipmaps = (0..levels)
        .map(|level| {
            let width = mip_dimension(header.width, level);
            let height = mip_dimension(header.height, level);
            let data = input.read_bytes(format.data_size(width, height))?;
            Ok(Mipmap {
                width,
                height,
                data_type: MipmapType::COPY,
                compressed_data_size: data.len() as u32,
                data,
            })
        })
        .collect::<Result<Vec<_>, AffError>>()?;
    mipmaps.reverse();
    Ok(RawEdds {
        header,
        format,
        mipmaps,
    })
}

/// Size of the mipmap `level` (`0` is the full size).
fn mip_dimension(size: u32, level: u32) -> usize {
    size.checked_shr(level).unwrap_or(0).max(1) as usize
}

fn decompress_lz4_chain(src: &[u8]) -> Option<Vec<u8>> {
    let size = u32::from_le_bytes(src.get(..4)?.try_into().ok()?) as usize;
    let mut src = &src[4..];
    let mut out = Vec::with_capacity(size);

    loop {
        let header = src.get(..4)?;
        let block_size =
            usize::from(header[0]) | (usize::from(header[1]) << 8) | (usize::from(header[2]) << 16);
        let is_last = header[3] != 0;
        let block = src.get(4..4 + block_size)?;
        src = &src[4 + block_size..];

        let unpacked_size = if is_last {
            size.checked_sub(out.len())?
        } else {
            LZ4_BLOCK_SIZE
        };
        let unpacked = lz4_flex::block::decompress_with_dict(block, unpacked_size, &out).ok()?;
        out.extend_from_slice(&unpacked);

        if is_last {
            return (out.len() == size).then_some(out);
        }
    }
}
