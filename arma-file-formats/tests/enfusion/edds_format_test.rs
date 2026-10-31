//! EDDS format tests on synthetic files, built in memory from blocks with known colors.

use std::io::Cursor;

use arma_file_formats::{
    core::types::PixelType,
    enfusion::edds::{Edds, EddsFormat, RawEdds},
    errors::AffError,
};

const DXGI_R16G16B16A16_FLOAT: u32 = 10;
const DXGI_B8G8R8A8_UNORM_SRGB: u32 = 91;
const DXGI_BC4_UNORM: u32 = 80;
const DXGI_BC5_UNORM: u32 = 83;
const DXGI_BC7_UNORM: u32 = 98;
const DXGI_BC7_UNORM_SRGB: u32 = 99;

enum Pixels {
    FourCc(&'static [u8; 4]),
    Dxgi(u32),
    /// Bit count and R, G, B, A masks.
    Masks(u32, [u32; 4]),
}

fn dds_header(width: u32, height: u32, mipmaps: u32, pixels: &Pixels) -> Vec<u8> {
    // DDPF_FOURCC, or DDPF_RGB with DDPF_ALPHAPIXELS if there's an alpha mask.
    let (flags, four_cc, bits, masks) = match pixels {
        Pixels::FourCc(four_cc) => (0x4, **four_cc, 0, [0; 4]),
        Pixels::Dxgi(_) => (0x4, *b"DX10", 0, [0; 4]),
        Pixels::Masks(bits, masks) => (0x40 | u32::from(masks[3] != 0), [0; 4], *bits, *masks),
    };
    let mut out = b"DDS ".to_vec();
    for value in [124, 0x0002_1007, height, width, 0, 0, mipmaps] {
        out.extend(u32::to_le_bytes(value));
    }
    out.extend([0; 11 * 4]);
    out.extend(32u32.to_le_bytes());
    out.extend(flags.to_le_bytes());
    out.extend(four_cc);
    out.extend(bits.to_le_bytes());
    for mask in masks {
        out.extend(mask.to_le_bytes());
    }
    for value in [0x1000u32, 0, 0, 0, 0] {
        out.extend(value.to_le_bytes());
    }
    if let Pixels::Dxgi(format) = pixels {
        for value in [*format, 3, 0, 1, 0] {
            out.extend(value.to_le_bytes());
        }
    }
    out
}

/// An EDDS with `COPY` mipmaps, given largest first like `mip_chain` returns them.
fn edds(width: u32, height: u32, pixels: &Pixels, mipmaps: &[Vec<u8>]) -> Vec<u8> {
    let mut out = dds_header(width, height, mipmaps.len() as u32, pixels);
    for mip in mipmaps.iter().rev() {
        out.extend(b"COPY");
        out.extend((mip.len() as u32).to_le_bytes());
    }
    for mip in mipmaps.iter().rev() {
        out.extend(mip);
    }
    out
}

fn decode(bytes: &[u8]) -> Result<Edds, AffError> {
    Edds::from(&mut Cursor::new(bytes))
}

/// BC1 block: `color0`/`color1` as RGB565, then a 2 bit index per pixel.
fn bc1_block(color0: u16, color1: u16, indices: [u8; 16]) -> Vec<u8> {
    let bits = indices
        .iter()
        .enumerate()
        .fold(0u32, |acc, (i, &index)| acc | (u32::from(index) << (2 * i)));
    [color0.to_le_bytes(), color1.to_le_bytes()]
        .concat()
        .into_iter()
        .chain(bits.to_le_bytes())
        .collect()
}

/// BC4 block (and the alpha half of BC3): two endpoints, then a 3 bit index per pixel.
fn bc4_block(value0: u8, value1: u8, indices: [u8; 16]) -> Vec<u8> {
    let bits = indices
        .iter()
        .enumerate()
        .fold(0u64, |acc, (i, &index)| acc | (u64::from(index) << (3 * i)));
    [value0, value1]
        .into_iter()
        .chain(bits.to_le_bytes().into_iter().take(6))
        .collect()
}

/// BC7 mode 6 block with equal endpoints, every pixel gets `(value << 1) | p_bit`.
fn bc7_solid_block(rgba: [u8; 4], p_bit: u8) -> Vec<u8> {
    let mut bits = 1u128 << 6;
    let mut position = 7;
    let mut push = |value: u128, count: u32| {
        bits |= value << position;
        position += count;
    };
    for channel in rgba {
        push(u128::from(channel), 7);
        push(u128::from(channel), 7);
    }
    push(u128::from(p_bit), 1);
    push(u128::from(p_bit), 1);
    bits.to_le_bytes().to_vec()
}

const RED_565: u16 = 0xF800;
const BLUE_565: u16 = 0x001F;

#[test]
fn bc1_decodes_and_crops_partial_blocks() {
    // 6x2: two blocks side by side, the second one is only partially inside the image.
    let mut indices = [0u8; 16];
    indices[1] = 1; // second pixel of the first row is color1
    let data = [
        bc1_block(RED_565, BLUE_565, indices),
        bc1_block(BLUE_565, RED_565, [0; 16]),
    ]
    .concat();
    let edds = decode(&edds(6, 2, &Pixels::FourCc(b"DXT1"), &[data])).unwrap();

    assert_eq!(edds.format, EddsFormat::Bc1 { srgb: false });
    assert_eq!(edds.pixel_type, PixelType::Rgba);
    let pixels = &edds.mipmaps[0].data;
    assert_eq!(pixels.len(), 6 * 2 * 4);
    assert_eq!(&pixels[0..4], &[255, 0, 0, 255]);
    assert_eq!(&pixels[4..8], &[0, 0, 255, 255]);
    assert_eq!(
        &pixels[16..20],
        &[0, 0, 255, 255],
        "x = 4 comes from the second block"
    );
    assert_eq!(
        &pixels[24..28],
        &[255, 0, 0, 255],
        "second row starts with the first block"
    );
}

#[test]
fn dxt5_is_bc3() {
    let mut alpha = [0u8; 16];
    alpha[0] = 1; // first pixel uses alpha1 = 0
    let block = [
        bc4_block(255, 0, alpha),
        bc1_block(RED_565, BLUE_565, [0; 16]),
    ]
    .concat();
    let edds = decode(&edds(4, 4, &Pixels::FourCc(b"DXT5"), &[block])).unwrap();

    assert_eq!(edds.format, EddsFormat::Bc3 { srgb: false });
    assert_eq!(&edds.mipmaps[0].data[0..8], &[255, 0, 0, 0, 255, 0, 0, 255]);
}

#[test]
fn bc4_and_bc5() {
    let bc4 = decode(&edds(
        4,
        4,
        &Pixels::Dxgi(DXGI_BC4_UNORM),
        &[bc4_block(200, 100, [0; 16])],
    ))
    .unwrap();
    assert_eq!(bc4.pixel_type, PixelType::Gray);
    assert_eq!(bc4.mipmaps[0].data, vec![200; 16]);

    let block = [bc4_block(200, 100, [0; 16]), bc4_block(50, 10, [0; 16])].concat();
    let bc5 = decode(&edds(4, 4, &Pixels::Dxgi(DXGI_BC5_UNORM), &[block])).unwrap();
    assert_eq!(bc5.format, EddsFormat::Bc5);
    assert_eq!(
        &bc5.mipmaps[0].data[..8],
        &[200, 50, 0, 255, 200, 50, 0, 255]
    );
}

#[test]
fn bc7_linear_and_srgb() {
    let block = bc7_solid_block([100, 50, 10, 127], 1);
    for (dxgi, srgb) in [(DXGI_BC7_UNORM, false), (DXGI_BC7_UNORM_SRGB, true)] {
        let edds = decode(&edds(4, 4, &Pixels::Dxgi(dxgi), &[block.clone()])).unwrap();
        assert_eq!(edds.format, EddsFormat::Bc7 { srgb });
        assert_eq!(edds.format.is_srgb(), srgb);
        assert!(
            edds.mipmaps[0]
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [201, 101, 21, 255])
        );
    }
}

#[test]
fn uncompressed_formats_are_swizzled_to_rgba() {
    let bgra = vec![10, 20, 30, 40];
    let edds_bgra = decode(&edds(
        1,
        1,
        &Pixels::Dxgi(DXGI_B8G8R8A8_UNORM_SRGB),
        &[bgra],
    ))
    .unwrap();
    assert_eq!(edds_bgra.format, EddsFormat::Bgra8 { srgb: true });
    assert_eq!(edds_bgra.mipmaps[0].data, [30, 20, 10, 40]);

    let argb_masks = Pixels::Masks(32, [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000]);
    let legacy = decode(&edds(1, 1, &argb_masks, &[vec![10, 20, 30, 40]])).unwrap();
    assert_eq!(legacy.format, EddsFormat::Bgra8 { srgb: false });
    assert_eq!(legacy.mipmaps[0].data, [30, 20, 10, 40]);

    let l8 = decode(&edds(
        2,
        1,
        &Pixels::Masks(8, [0xFF, 0, 0, 0]),
        &[vec![7, 9]],
    ))
    .unwrap();
    assert_eq!(l8.pixel_type, PixelType::Gray);
    assert_eq!(l8.mipmaps[0].data, [7, 9]);
}

#[test]
fn legacy_abgr_masks_are_rgba() {
    // Like the terrain's `shoreDFAtlas.edds`: no DX10 header, R in the lowest byte.
    let abgr_masks = Pixels::Masks(32, [0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000]);
    let edds = decode(&edds(
        2,
        1,
        &abgr_masks,
        &[vec![255, 255, 241, 184, 1, 2, 3, 4]],
    ))
    .unwrap();

    assert_eq!(edds.format, EddsFormat::Rgba8 { srgb: false });
    assert_eq!(edds.pixel_type, PixelType::Rgba);
    assert_eq!(edds.mipmaps[0].data, [255, 255, 241, 184, 1, 2, 3, 4]);
}

#[test]
fn tall_r8_with_odd_mipmaps_and_lz4_level() {
    // Like the river `*_flow.edds`: legacy 8 bit R mask, 4 pixels wide, a height that halves to
    // odd sizes and one `LZ4 ` level between `COPY` levels.
    let sizes = [(4, 108), (2, 54), (1, 27), (1, 13), (1, 6), (1, 3), (1, 1)];
    let levels: Vec<Vec<u8>> = sizes
        .iter()
        .enumerate()
        .map(|(level, (w, h))| (0..w * h).map(|i| (level * 30 + i % 7) as u8).collect())
        .collect();

    let mut file = dds_header(4, 108, 7, &Pixels::Masks(8, [0xFF, 0, 0, 0]));
    let mut stored = Vec::new();
    for (level, pixels) in levels.iter().enumerate().rev() {
        if level == 2 {
            let block = lz4_flex::block::compress(pixels);
            let mut lz4 = (pixels.len() as u32).to_le_bytes().to_vec();
            lz4.extend(&(block.len() as u32).to_le_bytes()[..3]);
            lz4.push(1);
            lz4.extend(block);
            file.extend(b"LZ4 ");
            file.extend((lz4.len() as u32).to_le_bytes());
            stored.extend(lz4);
        } else {
            file.extend(b"COPY");
            file.extend((pixels.len() as u32).to_le_bytes());
            stored.extend(pixels);
        }
    }
    file.extend(stored);
    let edds = decode(&file).unwrap();

    assert_eq!(edds.format, EddsFormat::L8);
    assert_eq!(edds.pixel_type, PixelType::Gray);
    let decoded: Vec<_> = edds
        .mipmaps
        .iter()
        .rev()
        .map(|m| (m.width, m.height))
        .collect();
    assert_eq!(decoded, sizes);
    for (mipmap, pixels) in edds.mipmaps.iter().rev().zip(&levels) {
        assert_eq!(&mipmap.data, pixels, "{}x{}", mipmap.width, mipmap.height);
    }
}

#[test]
fn mip_chain_down_to_one_pixel() {
    let block = bc1_block(RED_565, BLUE_565, [0; 16]);
    let chain = vec![block.clone(), block.clone(), block];
    let raw = RawEdds::from(&mut Cursor::new(edds(
        4,
        4,
        &Pixels::FourCc(b"DXT1"),
        &chain,
    )))
    .unwrap();

    let sizes: Vec<_> = raw.mipmaps.iter().map(|m| (m.width, m.height)).collect();
    assert_eq!(sizes, [(1, 1), (2, 2), (4, 4)], "smallest first");
    assert_eq!(
        raw.mipmaps[0].data.len(),
        8,
        "raw mipmaps stay block compressed"
    );
    assert_eq!(raw.decode_mipmap(0).unwrap(), [255, 0, 0, 255]);
    assert_eq!(raw.decode_mipmap(1).unwrap().len(), 2 * 2 * 4);
    assert!(raw.decode_mipmap(3).is_err());
}

#[test]
fn lz4_block_chain() {
    // 256x128 BGRA is 128 KiB: two 64 KiB LZ4 blocks, the second uses the first as dictionary.
    let pixels: Vec<u8> = (0..256 * 128 * 4)
        .map(|i: u32| (i / 7 % 251) as u8)
        .collect();
    let (first, second) = pixels.split_at(0x10000);
    let mut stored = (pixels.len() as u32).to_le_bytes().to_vec();
    for (block, is_last) in [
        (lz4_flex::block::compress(first), false),
        (lz4_flex::block::compress_with_dict(second, first), true),
    ] {
        stored.extend(&(block.len() as u32).to_le_bytes()[..3]);
        stored.push(u8::from(is_last));
        stored.extend(block);
    }

    let mut file = dds_header(256, 128, 1, &Pixels::Dxgi(DXGI_B8G8R8A8_UNORM_SRGB));
    file.extend(b"LZ4 ");
    file.extend((stored.len() as u32).to_le_bytes());
    file.extend(&stored);
    let raw = RawEdds::from(&mut Cursor::new(&file)).unwrap();
    assert_eq!(raw.mipmaps[0].data, pixels);

    // Cut into the second block: an error, not a panic.
    file.truncate(file.len() - 100);
    let mut truncated = dds_header(256, 128, 1, &Pixels::Dxgi(DXGI_B8G8R8A8_UNORM_SRGB));
    truncated.extend(b"LZ4 ");
    truncated.extend((stored.len() as u32 - 100).to_le_bytes());
    truncated.extend(&stored[..stored.len() - 100]);
    assert!(RawEdds::from(&mut Cursor::new(&truncated)).is_err());
    assert!(RawEdds::from(&mut Cursor::new(&file)).is_err());
}

#[test]
fn plain_dds_without_mipmap_table() {
    let mut file = dds_header(8, 8, 2, &Pixels::Dxgi(DXGI_BC4_UNORM));
    file.extend(bc4_block(10, 0, [0; 16]).repeat(4)); // 8x8, largest first
    file.extend(bc4_block(20, 0, [0; 16])); // 4x4
    let edds = decode(&file).unwrap();

    assert_eq!(edds.mipmaps[0].width, 4);
    assert_eq!(edds.mipmaps[0].data, vec![20; 16]);
    assert_eq!(edds.mipmaps[1].data, vec![10; 64]);
}

#[test]
fn unsupported_format_is_an_error() {
    let file = edds(1, 1, &Pixels::Dxgi(DXGI_R16G16B16A16_FLOAT), &[vec![0; 8]]);
    assert!(matches!(
        decode(&file),
        Err(AffError::UnknownImageDataFormat(_))
    ));
}

#[test]
fn mipmap_too_short_is_an_error() {
    let file = edds(4, 4, &Pixels::FourCc(b"DXT1"), &[vec![0; 4]]);
    assert!(decode(&file).is_err());
}
