use std::io::{Read, Seek};

use binrw::BinRead;
use lzzzz::lz4;

use crate::{
    core::read::ReadExtTrait,
    enfusion::edds::{
        dds_header::DdsHeaderDX10, DdsHeader, DdsPixelFormatEnum, DxgiFormat, FourCCEnum,
    },
    errors::AffError,
};

#[derive(Debug, Clone)]
pub struct Edds {
    pub header: DdsHeader,
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

impl Edds {
    pub fn from<I>(input: &mut I) -> Result<Edds, AffError>
    where
        I: Seek + Read,
    {
        let header = DdsHeader::read(input)?;
        let mut mipmaps = Vec::new();

        for i in (1..(header.mipmap_count + 1)).rev() {
            let data_type = input.read_string_lossy(4)?;
            let compressed_data_size = input.read_u32()?;
            mipmaps.push(Mipmap {
                width: Edds::get_dim_for_index(header.width, i),
                height: Edds::get_dim_for_index(header.height, i),
                data_type: match data_type.as_str() {
                    "COPY" => MipmapType::COPY,
                    "LZ4 " => MipmapType::LZ4,
                    unk => return Err(AffError::UnknownImageDataType(format!("{:?}", unk))),
                },
                data: Vec::new(),
                compressed_data_size,
            });
        }

        for mipmap in mipmaps.iter_mut() {
            match mipmap.data_type {
                MipmapType::COPY => {
                    let mut buf = vec![0; mipmap.compressed_data_size as usize];
                    input.read_exact(&mut buf).unwrap();
                    mipmap.data = Edds::decode_data(&buf, mipmap.width, mipmap.height, &header)?;
                }
                MipmapType::LZ4 => {
                    let mut lz4_stream = lz4::Decompressor::new().unwrap();

                    let uncompressed_data_size = input.read_u32().unwrap() as usize;

                    let mut data_read = 4;
                    let mut complete_buffer = Vec::with_capacity(uncompressed_data_size as usize);

                    loop {
                        let compress_block_size = input.read_u24().unwrap() as usize;
                        data_read += 3;

                        let is_last_block = input.read_u8().unwrap() as u32 != 0;
                        data_read += 1;

                        let mut buf = vec![0; compress_block_size];
                        input.read_exact(&mut buf).unwrap();

                        data_read += compress_block_size;

                        let mut block_size = 65536;
                        if is_last_block {
                            block_size = uncompressed_data_size - complete_buffer.len();
                        }

                        let decomp = lz4_stream.next(&buf, block_size as usize).unwrap();
                        complete_buffer.append(&mut decomp.to_owned());

                        if is_last_block {
                            assert_eq!(data_read, mipmap.compressed_data_size as usize);
                            break;
                        }
                    }

                    mipmap.data =
                        Edds::decode_data(&complete_buffer, mipmap.width, mipmap.height, &header)?;
                }
            };
        }

        Ok(Edds { header, mipmaps })
    }

    fn get_dim_for_index(max_dim: u32, index: u32) -> usize {
        std::cmp::max(max_dim / 2_u32.pow(index - 1), 1) as usize
    }

    fn decode_data(
        src: &[u8],
        width: usize,
        height: usize,
        header: &DdsHeader,
    ) -> Result<Vec<u8>, AffError> {
        match &header.dx10_header {
            Some(dx10_header) => decode_dx10_data(dx10_header, src, width, height),
            None => decode_four_cc_data(header, src, width, height),
        }
    }
}

fn decode_four_cc_data(
    header: &DdsHeader,
    src: &[u8],
    width: usize,
    height: usize,
) -> Result<Vec<u8>, AffError> {
    match &header.pixel_format.four_cc {
        FourCCEnum::None => decode_pixel_format_data(header, src),
        FourCCEnum::DXT5 => {
            let bc5 = texpresso::Format::Bc5;
            let mut output = vec![0; width * height * 4];
            bc5.decompress(src, width, height, &mut output);
            Ok(output)
        }
        ni_four_cc => Err(AffError::UnknownImageDataFormat(format!(
            "{:?}",
            ni_four_cc
        ))),
    }
}

fn decode_pixel_format_data(header: &DdsHeader, src: &[u8]) -> Result<Vec<u8>, AffError> {
    match header.get_pixel_format() {
        DdsPixelFormatEnum::D3DFMT_X8R8G8B8 | DdsPixelFormatEnum::D3DFMT_A8R8G8B8 => {
            let mut src = src.to_vec();
            for i in (0..src.len()).step_by(4) {
                let r = src[i];
                let b = src[i + 2];

                src[i] = b;
                src[i + 2] = r;
            }
            Ok(src)
        }
        unk => Err(AffError::UnknownImageDataFormat(format!("{:?}", unk))),
    }
}

fn decode_dx10_data(
    dx10_header: &DdsHeaderDX10,
    src: &[u8],
    width: usize,
    height: usize,
) -> Result<Vec<u8>, AffError> {
    match dx10_header.dxgi_format {
        DxgiFormat::DXGI_FORMAT_BC4_UNORM => {
            block_decompression(src, width, height, BCCompressionType::BC4)
        }
        DxgiFormat::DXGI_FORMAT_B8G8R8X8_UNORM_SRGB => {
            let mut src = src.to_vec();
            for i in (0..src.len()).step_by(4) {
                let r = src[i];
                let b = src[i + 2];

                src[i] = b;
                src[i + 2] = r;
            }
            Ok(src)
        }
        DxgiFormat::DXGI_FORMAT_BC7_UNORM_SRGB => {
            block_decompression(src, width, height, BCCompressionType::BC7)
        }
        _ => Err(AffError::UnknownImageDataFormat(format!(
            "{:?}",
            dx10_header.dxgi_format
        ))),
    }
}

fn block_decompression(
    src: &[u8],
    width: usize,
    height: usize,
    compression_type: BCCompressionType,
) -> Result<Vec<u8>, AffError> {
    let mut dst = vec![0_u8; width * height * compression_type.pixel_size()];

    let mut small_block = false;

    if dst.len() < compression_type.smallest_block() {
        dst = vec![0_u8; compression_type.smallest_block()];
        small_block = true;
    }

    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let block_byte_size = compression_type.block_byte_size() as usize;
    let output_row_pitch = width as usize * compression_type.pixel_size();

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_index = (by * blocks_x + bx) as usize;
            let block_offset = block_index * block_byte_size;

            if block_offset + block_byte_size > src.len() {
                break;
            }

            let output_offset =
                (by * 4 * output_row_pitch + bx * compression_type.pixel_size() * 4) as usize;

            if output_offset < dst.len() {
                compression_type.decode_block(
                    &src[block_offset..block_offset + block_byte_size],
                    &mut dst[output_offset..],
                    output_row_pitch,
                );
            }
        }
    }

    if small_block {
        dst.resize(width * height * compression_type.pixel_size(), 0);
    }

    Ok(dst)
}

enum BCCompressionType {
    BC4,
    BC7,
}

impl BCCompressionType {
    const fn block_byte_size(&self) -> u32 {
        match self {
            Self::BC4 => 8,
            Self::BC7 => 16,
        }
    }

    const fn pixel_size(&self) -> usize {
        match self {
            Self::BC4 => 1,
            Self::BC7 => 4,
        }
    }

    const fn smallest_block(&self) -> usize {
        (self.block_byte_size() as usize) * 4
    }

    fn decode_block(
        &self,
        compressed_block: &[u8],
        decompressed_block: &mut [u8],
        destination_pitch: usize,
    ) {
        match self {
            Self::BC4 => block_compression::decode::decode_block_bc4(
                compressed_block,
                decompressed_block,
                destination_pitch,
            ),
            Self::BC7 => block_compression::decode::decode_block_bc7(
                compressed_block,
                decompressed_block,
                destination_pitch,
            ),
        };
    }
}
