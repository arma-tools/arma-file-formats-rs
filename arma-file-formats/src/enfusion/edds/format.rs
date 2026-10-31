use crate::{
    core::types::PixelType,
    enfusion::edds::{DdsHeader, DdsPixelFormatEnum, DxgiFormat, FourCCEnum},
    errors::AffError,
};

/// Layout of the pixel data stored in the mipmaps of an EDDS texture.
///
/// Block compressed formats store 4x4 pixel blocks, mipmaps smaller than a block still take a
/// whole block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EddsFormat {
    /// RGB with 1 bit alpha (`DXT1`), 8 bytes per block.
    Bc1 { srgb: bool },
    /// RGB with explicit 4 bit alpha (`DXT3`), 16 bytes per block.
    Bc2 { srgb: bool },
    /// RGB with interpolated alpha (`DXT5`), 16 bytes per block.
    Bc3 { srgb: bool },
    /// Single channel, 8 bytes per block.
    Bc4,
    /// Two channels, typically a normal map's X and Y, 16 bytes per block.
    Bc5,
    /// Unsigned half float RGB (HDR), 16 bytes per block.
    Bc6hUf16,
    /// RGBA, 16 bytes per block.
    Bc7 { srgb: bool },
    /// 8 bit RGBA.
    Rgba8 { srgb: bool },
    /// 8 bit BGRA.
    Bgra8 { srgb: bool },
    /// 8 bit BGR followed by an unused byte.
    Bgrx8 { srgb: bool },
    /// 8 bit luminance.
    L8,
}

impl EddsFormat {
    /// Determines the format from the DX10 header, or the legacy `FourCC` / pixel format masks.
    pub fn from_header(header: &DdsHeader) -> Result<Self, AffError> {
        let unsupported = || AffError::UnknownImageDataFormat(describe(header));
        if let Some(dx10) = &header.dx10_header {
            return Ok(match dx10.dxgi_format {
                DxgiFormat::DXGI_FORMAT_BC1_UNORM => Self::Bc1 { srgb: false },
                DxgiFormat::DXGI_FORMAT_BC1_UNORM_SRGB => Self::Bc1 { srgb: true },
                DxgiFormat::DXGI_FORMAT_BC2_UNORM => Self::Bc2 { srgb: false },
                DxgiFormat::DXGI_FORMAT_BC2_UNORM_SRGB => Self::Bc2 { srgb: true },
                DxgiFormat::DXGI_FORMAT_BC3_UNORM => Self::Bc3 { srgb: false },
                DxgiFormat::DXGI_FORMAT_BC3_UNORM_SRGB => Self::Bc3 { srgb: true },
                DxgiFormat::DXGI_FORMAT_BC4_UNORM => Self::Bc4,
                DxgiFormat::DXGI_FORMAT_BC5_UNORM => Self::Bc5,
                DxgiFormat::DXGI_FORMAT_BC6H_UF16 => Self::Bc6hUf16,
                DxgiFormat::DXGI_FORMAT_BC7_UNORM => Self::Bc7 { srgb: false },
                DxgiFormat::DXGI_FORMAT_BC7_UNORM_SRGB => Self::Bc7 { srgb: true },
                DxgiFormat::DXGI_FORMAT_R8G8B8A8_UNORM => Self::Rgba8 { srgb: false },
                DxgiFormat::DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => Self::Rgba8 { srgb: true },
                DxgiFormat::DXGI_FORMAT_B8G8R8A8_UNORM => Self::Bgra8 { srgb: false },
                DxgiFormat::DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => Self::Bgra8 { srgb: true },
                DxgiFormat::DXGI_FORMAT_B8G8R8X8_UNORM => Self::Bgrx8 { srgb: false },
                DxgiFormat::DXGI_FORMAT_B8G8R8X8_UNORM_SRGB => Self::Bgrx8 { srgb: true },
                DxgiFormat::DXGI_FORMAT_R8_UNORM => Self::L8,
                _ => return Err(unsupported()),
            });
        }

        Ok(match header.pixel_format.four_cc {
            FourCCEnum::DXT1 => Self::Bc1 { srgb: false },
            FourCCEnum::DXT2 | FourCCEnum::DXT3 => Self::Bc2 { srgb: false },
            FourCCEnum::DXT4 | FourCCEnum::DXT5 => Self::Bc3 { srgb: false },
            FourCCEnum::BC4U => Self::Bc4,
            FourCCEnum::BC5U => Self::Bc5,
            FourCCEnum::None => match header.get_pixel_format() {
                DdsPixelFormatEnum::D3DFMT_A8R8G8B8 => Self::Bgra8 { srgb: false },
                DdsPixelFormatEnum::D3DFMT_X8R8G8B8 => Self::Bgrx8 { srgb: false },
                DdsPixelFormatEnum::D3DFMT_A8B8G8R8 => Self::Rgba8 { srgb: false },
                DdsPixelFormatEnum::D3DFMT_L8 => Self::L8,
                _ => return Err(unsupported()),
            },
            _ => return Err(unsupported()),
        })
    }

    /// Whether the color channels are sRGB encoded.
    #[must_use]
    pub const fn is_srgb(self) -> bool {
        match self {
            Self::Bc1 { srgb }
            | Self::Bc2 { srgb }
            | Self::Bc3 { srgb }
            | Self::Bc7 { srgb }
            | Self::Rgba8 { srgb }
            | Self::Bgra8 { srgb }
            | Self::Bgrx8 { srgb } => srgb,
            Self::Bc4 | Self::Bc5 | Self::Bc6hUf16 | Self::L8 => false,
        }
    }

    /// Bytes per 4x4 block, `None` for uncompressed formats.
    #[must_use]
    pub const fn block_size(self) -> Option<usize> {
        match self {
            Self::Bc1 { .. } | Self::Bc4 => Some(8),
            Self::Bc2 { .. } | Self::Bc3 { .. } | Self::Bc5 | Self::Bc6hUf16 | Self::Bc7 { .. } => {
                Some(16)
            }
            Self::Rgba8 { .. } | Self::Bgra8 { .. } | Self::Bgrx8 { .. } | Self::L8 => None,
        }
    }

    /// Size of a `width` x `height` image in this format.
    #[must_use]
    pub const fn data_size(self, width: usize, height: usize) -> usize {
        if let Some(block) = self.block_size() {
            return width.div_ceil(4) * height.div_ceil(4) * block;
        }
        let bytes_per_pixel = match self {
            Self::L8 => 1,
            _ => 4,
        };
        width * height * bytes_per_pixel
    }

    /// Layout of the data [`EddsFormat::decode`] produces: [`PixelType::Gray`] (1 byte per
    /// pixel) for single channel formats, [`PixelType::Rgba`] (4 bytes) for everything else.
    #[must_use]
    pub const fn decoded_pixel_type(self) -> PixelType {
        match self {
            Self::Bc4 | Self::L8 => PixelType::Gray,
            _ => PixelType::Rgba,
        }
    }

    /// Decodes `data` (at least [`EddsFormat::data_size`] bytes) to 8 bit pixels, see
    /// [`EddsFormat::decoded_pixel_type`].
    ///
    /// BC5 is expanded to RGBA with blue `0` and alpha `255`. BC6H is clamped to `0..=1`
    /// without tone mapping, it's only meant for previews.
    pub fn decode(self, data: &[u8], width: usize, height: usize) -> Result<Vec<u8>, AffError> {
        let size = self.data_size(width, height);
        let data = data.get(..size).ok_or_else(|| {
            AffError::ParseError(format!(
                "{self:?} image of {width}x{height} needs {size} bytes, got {}",
                data.len()
            ))
        })?;

        Ok(match self {
            Self::Rgba8 { .. } | Self::L8 => data.to_vec(),
            Self::Bgra8 { .. } => data
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|&[b, g, r, a]| [r, g, b, a])
                .collect(),
            Self::Bgrx8 { .. } => data
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|&[b, g, r, _]| [r, g, b, 255])
                .collect(),
            Self::Bc4 => decode_blocks::<1>(data, width, height, 8, |block, out| {
                block_compression::decode::decode_block_bc4(block, out, 4);
            }),
            Self::Bc1 { .. } => decode_blocks::<4>(data, width, height, 8, |block, out| {
                block_compression::decode::decode_block_bc1(block, out, 16);
            }),
            Self::Bc2 { .. } => decode_blocks::<4>(data, width, height, 16, |block, out| {
                block_compression::decode::decode_block_bc2(block, out, 16);
            }),
            Self::Bc3 { .. } => decode_blocks::<4>(data, width, height, 16, |block, out| {
                block_compression::decode::decode_block_bc3(block, out, 16);
            }),
            Self::Bc7 { .. } => decode_blocks::<4>(data, width, height, 16, |block, out| {
                block_compression::decode::decode_block_bc7(block, out, 16);
            }),
            Self::Bc5 => decode_blocks::<4>(data, width, height, 16, |block, out| {
                let mut rg = [0u8; 32];
                block_compression::decode::decode_block_bc5(block, &mut rg, 8);
                for (pixel, [r, g]) in out
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(rg.as_chunks::<2>().0)
                {
                    *pixel = [*r, *g, 0, 255];
                }
            }),
            Self::Bc6hUf16 => decode_blocks::<4>(data, width, height, 16, |block, out| {
                let mut rgba = [half::f16::ZERO; 64];
                block_compression::decode::decode_block_bc6h(block, &mut rgba, 16, false);
                for (byte, value) in out.iter_mut().zip(rgba) {
                    *byte = (value.to_f32().clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            }),
        })
    }
}

/// Decodes every block into a 4x4 tile with `CHANNELS` bytes per pixel, then copies the part
/// inside the image. Images that aren't a multiple of 4 (small mipmaps) are cropped.
fn decode_blocks<const CHANNELS: usize>(
    data: &[u8],
    width: usize,
    height: usize,
    block_size: usize,
    decode_block: impl Fn(&[u8], &mut [u8]),
) -> Vec<u8> {
    let mut out = vec![0u8; width * height * CHANNELS];
    let mut tile = [0u8; 64];
    let tile = &mut tile[..16 * CHANNELS];
    let blocks_x = width.div_ceil(4);

    for (index, block) in data.chunks_exact(block_size).enumerate() {
        let (bx, by) = (index % blocks_x * 4, index / blocks_x * 4);
        if by >= height {
            break;
        }
        decode_block(block, tile);
        let columns = (width - bx).min(4) * CHANNELS;
        for row in 0..(height - by).min(4) {
            let start = ((by + row) * width + bx) * CHANNELS;
            out[start..start + columns]
                .copy_from_slice(&tile[row * 4 * CHANNELS..row * 4 * CHANNELS + columns]);
        }
    }
    out
}

fn describe(header: &DdsHeader) -> String {
    header.dx10_header.as_ref().map_or_else(
        || match header.pixel_format.four_cc {
            FourCCEnum::None => format!("{:?}", header.get_pixel_format()),
            ref four_cc => format!("{four_cc:?}"),
        },
        |dx10| format!("{:?}", dx10.dxgi_format),
    )
}
