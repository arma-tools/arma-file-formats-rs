use std::{
    fs::{self, File},
    io::{BufReader, Cursor},
};

use arma_file_formats::{
    self,
    core::types::PixelType,
    errors::PaaError,
    real_virtuality::{
        paa::{Mipmap, Paa, Tagg},
        types::PaaType,
    },
};
use image::ImageBuffer;
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/paa_in/";
const OUTPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/paa_out/";

#[test]
fn default_test() {
    Paa::default();
}

#[test]
#[serial]
fn arma_test() {
    let file = File::open(format!("{}black_co_dxt1.paa", INPUT_PATH_PREFIX)).unwrap();
    let mut dxt1 = Paa::from_reader(&mut BufReader::new(file), None).unwrap();
    let mut mm_data = dxt1.mipmaps[0].data.clone();

    #[allow(clippy::needless_range_loop)]
    for i in 0..dxt1.mipmaps[0].width as usize * 100 * 4 {
        mm_data[i] = 0xFF;
    }

    dxt1.mipmaps[0].data = mm_data;

    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);

    dxt1.write(&mut cursor, None).unwrap();
    fs::write(format!("{}black_co.paa", OUTPUT_PATH_PREFIX), buf).unwrap();

    let file = File::open(format!("{}medic_cross_ca_dxt5.paa", INPUT_PATH_PREFIX)).unwrap();
    let mut dxt5 = Paa::from_reader(&mut BufReader::new(file), None).unwrap();

    let mut mm_data = dxt5.mipmaps[0].data.clone();

    #[allow(clippy::needless_range_loop)]
    for i in 0..dxt5.mipmaps[0].width as usize * 10 * 4 {
        mm_data[i] = 0xFF;
    }

    dxt5.mipmaps[0].data = mm_data;

    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);

    dxt5.write(&mut cursor, None).unwrap();
    fs::write(format!("{}medic_cross_ca.paa", OUTPUT_PATH_PREFIX), buf).unwrap();
}

#[test]
fn tagg_default_test() {
    let tagg = Tagg::default();
    assert!(tagg.data.is_empty());
    assert!(tagg.signature.is_empty());
}

#[test]
fn max_mipmap_count_test() {
    assert_eq!(Paa::max_mipmap_count(1024, 512), 10);
    assert_eq!(Paa::max_mipmap_count(512, 1024), 10);
    assert_eq!(Paa::max_mipmap_count(128, 32), 7);
}

#[test]
fn dim_at_level_test() {
    assert_eq!(Paa::dim_at_level(1024, 10), 1);
    assert_eq!(Paa::dim_at_level(1024, 1), 512);
    assert_eq!(Paa::dim_at_level(1024, 0), 1024);
    assert_eq!(Paa::dim_at_level(32, 6), 1);
}

#[test]
#[serial]
fn logo_dxt5_128_decoding() {
    let file = File::open(format!("{}logo_dxt5_128.paa", INPUT_PATH_PREFIX)).unwrap();
    let paa = Paa::from_reader(&mut BufReader::new(file), None).unwrap();

    let mm = paa.mipmaps.first().unwrap();

    let img_buf: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(mm.width.into(), mm.height.into(), mm.data.clone()).unwrap();
    img_buf
        .save(format!("{}logo_dxt5_128.png", OUTPUT_PATH_PREFIX))
        .unwrap();

    assert_eq!(paa.pixel_type, PixelType::Rgba);
}

#[test]
#[serial]
fn logo_dxt1_2048_decoding() {
    let file = File::open(format!("{}logo_dxt1_2048.paa", INPUT_PATH_PREFIX)).unwrap();
    let paa = Paa::from_reader(&mut BufReader::new(file), Some(&[0])).unwrap();

    let mm = paa.mipmaps.first().unwrap();

    let img_buf: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(mm.width.into(), mm.height.into(), mm.data.clone()).unwrap();
    img_buf
        .save(format!("{}logo_dxt1_2048.png", OUTPUT_PATH_PREFIX))
        .unwrap();
    assert_eq!(paa.pixel_type, PixelType::Rgba);
}

#[test]
#[serial]
fn logo_dxt5_400_decoding() {
    let file = File::open(format!("{}logo_dxt5_400.paa", INPUT_PATH_PREFIX)).unwrap();
    let paa = Paa::from_reader(&mut BufReader::new(file), Some(&[0])).unwrap();

    let mm = paa.mipmaps.first().unwrap();

    let img_buf: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(mm.width.into(), mm.height.into(), mm.data.clone()).unwrap();
    img_buf
        .save(format!("{}logo_dxt5_400.png", OUTPUT_PATH_PREFIX))
        .unwrap();
    assert_eq!(paa.pixel_type, PixelType::Rgba);
}

#[test]
#[serial]
fn logo_dxt5_128_encoding() {
    let img = image::open(format!("{}logo_dxt5_128.png", INPUT_PATH_PREFIX)).unwrap();

    let mut paa = Paa::from_image(
        img.width() as u16,
        img.height() as u16,
        img.as_bytes().to_vec(),
    );

    let mut out_file = File::create(format!("{}logo_dxt5_128.paa", OUTPUT_PATH_PREFIX)).unwrap();

    paa.write(&mut out_file, None).unwrap();
}

#[test]
#[serial]
fn logo_dxt1_2048_encoding() {
    let img = image::open(format!("{}logo_dxt1_2048.png", INPUT_PATH_PREFIX)).unwrap();

    let mut paa = Paa::from_image(
        img.width() as u16,
        img.height() as u16,
        img.as_bytes().to_vec(),
    );

    let mut out_file = File::create(format!("{}logo_dxt1_2048.paa", OUTPUT_PATH_PREFIX)).unwrap();

    paa.write(&mut out_file, None).unwrap();
}

#[test]
#[serial]
fn ai_88_plus_decoding() {
    let file = File::open(format!("{}ai88_plus.paa", INPUT_PATH_PREFIX)).unwrap();
    let paa = Paa::from_reader(&mut BufReader::new(file), Some(&[0])).unwrap();

    let mm = paa.mipmaps.first().unwrap();

    let img_buf: ImageBuffer<image::LumaA<u8>, Vec<u8>> =
        ImageBuffer::from_raw(mm.width.into(), mm.height.into(), mm.data.clone()).unwrap();

    img_buf
        .save(format!("{}ai88_plus.png", OUTPUT_PATH_PREFIX))
        .unwrap();

    assert_eq!(paa.pixel_type, PixelType::GrayAlpha);
}

#[test]
#[serial]
fn argb4444_staszow_decoding() {
    let file = File::open(format!(
        "{}argb4444_StaszowWinter_ca.paa",
        INPUT_PATH_PREFIX
    ))
    .unwrap();
    let paa = Paa::from_reader(&mut BufReader::new(file), Some(&[0])).unwrap();

    let mm = paa.mipmaps.first().unwrap();

    let img_buf: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(mm.width.into(), mm.height.into(), mm.data.clone()).unwrap();

    img_buf
        .save(format!(
            "{}argb4444_StaszowWinter_ca.png",
            OUTPUT_PATH_PREFIX
        ))
        .unwrap();

    assert_eq!(paa.pixel_type, PixelType::Rgba);
}

#[test]
#[serial]
fn rgba5551_djalka_decoding() {
    let file = File::open(format!("{}rgba5551_pictureMap_ca.paa", INPUT_PATH_PREFIX)).unwrap();
    let paa = Paa::from_reader(&mut BufReader::new(file), Some(&[0])).unwrap();

    let mm = paa.mipmaps.first().unwrap();

    let img_buf: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(mm.width.into(), mm.height.into(), mm.data.clone()).unwrap();

    img_buf
        .save(format!("{}rgba5551_pictureMap_ca.png", OUTPUT_PATH_PREFIX))
        .unwrap();

    assert_eq!(paa.pixel_type, PixelType::Rgba);
}

#[test]
fn blocks() {
    for (name, format, block_size) in [
        ("logo_dxt1_2048.paa", squish::Format::Bc1, 8),
        ("logo_dxt5_128.paa", squish::Format::Bc3, 16),
        ("medic_cross_ca_dxt5.paa", squish::Format::Bc3, 16),
    ] {
        let data = fs::read(format!("{}{}", INPUT_PATH_PREFIX, name)).unwrap();
        let blocks = Paa::from_reader_blocks(&mut Cursor::new(&data), None).unwrap();
        let decoded = Paa::from_reader(&mut Cursor::new(&data), None).unwrap();
        assert_eq!(blocks.mipmaps.len(), decoded.mipmaps.len());
        for (raw, rgba) in blocks.mipmaps.iter().zip(&decoded.mipmaps) {
            let (width, height) = (raw.width as usize, raw.height as usize);
            let block_count = width.div_ceil(4) * height.div_ceil(4);
            assert_eq!(
                raw.data.len(),
                block_count * block_size,
                "{name} {width}x{height}"
            );
            let mut pixels = vec![0; width * height * 4];
            format.decompress(&raw.data, width, height, &mut pixels);
            assert_eq!(pixels, rgba.data, "{name} {width}x{height}");
        }
    }
}

#[test]
fn blocks_other_formats() {
    // Formats other than DXT1 and DXT5 are decoded like `from_reader` does.
    let data = fs::read(format!(
        "{}argb4444_StaszowWinter_ca.paa",
        INPUT_PATH_PREFIX
    ))
    .unwrap();
    let blocks = Paa::from_reader_blocks(&mut Cursor::new(&data), Some(&[0])).unwrap();
    let decoded = Paa::from_reader(&mut Cursor::new(&data), Some(&[0])).unwrap();
    assert_eq!(blocks.mipmaps[0].data, decoded.mipmaps[0].data);
}

#[test]
fn uncompressed_4096_dxt5() {
    // An uncompressed 4096x4096 DXT5 mipmap has 2^24 bytes, its 24-bit size is 0.
    let big: Vec<u8> = (0..1u32 << 24).map(|i| (i % 251) as u8 + 1).collect();
    let small: Vec<u8> = (1..=16).collect();
    let mut data = vec![0x05, 0xff, 0, 0];
    for (size, blocks) in [(4096u16, &big), (4, &small)] {
        data.extend(size.to_le_bytes());
        data.extend(size.to_le_bytes());
        data.extend(&(blocks.len() as u32).to_le_bytes()[..3]);
        data.extend(blocks.iter());
    }
    data.extend([0; 6]);

    let paa = Paa::from_reader_blocks(&mut Cursor::new(&data), None).unwrap();
    assert_eq!(paa.mipmaps.len(), 2);
    assert_eq!(paa.mipmaps[0].data, big);
    assert_eq!(paa.mipmaps[1].data, small);
}

#[test]
fn not_paa() {
    let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    assert!(matches!(
        Paa::from_reader(&mut Cursor::new(png), None),
        Err(PaaError::Png)
    ));
    assert!(matches!(
        Paa::from_reader(&mut Cursor::new([0x34, 0x12, 0, 0]), None),
        Err(PaaError::UnknownType(0x1234))
    ));
}

/// A texture that ends before its mipmaps, like the decoys of obfuscated PBOs, fails as that.
#[test]
fn truncated() {
    let data = fs::read(format!("{}logo_dxt5_128.paa", INPUT_PATH_PREFIX)).unwrap();
    let paa = Paa::from_reader(&mut Cursor::new(&data), None).unwrap();
    // Cut into the last mipmap, without its end marker.
    let len = data.len() - 7;
    let err = Paa::from_reader_blocks(&mut Cursor::new(&data[..len]), None).unwrap_err();
    assert!(
        matches!(err, PaaError::MipmapPastEnd { index, len: l, .. } if index == paa.mipmaps.len() - 1 && l == len as u64),
        "{err:?}"
    );
}

/// A `.paa` of this type without tags and one 4x4 mipmap of 16 bytes.
fn small_paa(paa_type: u16) -> Vec<u8> {
    let mut paa = paa_type.to_le_bytes().to_vec();
    paa.extend([0, 0]);
    paa.extend([4, 0, 4, 0, 16, 0, 0]);
    paa.extend([0; 16]);
    paa.extend([0; 6]);
    paa
}

/// The 4 bytes of a tag, `None` without it.
fn tag(paa: &[u8], name: &[u8; 8]) -> Option<[u8; 4]> {
    let pos = paa.windows(8).position(|w| w == name)?;
    paa[pos + 12..pos + 16].try_into().ok()
}

#[test]
fn unsupported_types_are_errors() {
    for paa_type in [0xff02, 0xff03, 0xff04, 0x8888] {
        assert!(
            matches!(
                Paa::from_reader(&mut Cursor::new(small_paa(paa_type)), None),
                Err(PaaError::UnsupportedDecode(t)) if t == paa_type
            ),
            "{paa_type:#x}"
        );
    }
    for paa_type in [
        PaaType::DXT2,
        PaaType::RGBA4444,
        PaaType::ARGB1555,
        PaaType::RGBA8888,
        PaaType::GRAYwAlpha,
        PaaType::UNKNOWN,
    ] {
        let mut paa = Paa::from_image(4, 4, vec![255; 64]);
        assert!(
            matches!(
                paa.write(&mut Cursor::new(Vec::new()), Some(paa_type)),
                Err(PaaError::UnsupportedEncode(t)) if t == u16::from(paa_type)
            ),
            "{paa_type:?}"
        );
    }
}

#[test]
fn encoded_average_color_is_bgra() {
    // Opaque red: DXT1 without a FLAG tag, the average color blue first like the game's files.
    let mut paa = Paa::from_image(8, 8, [255, 0, 0, 255].repeat(64));
    let mut out = Cursor::new(Vec::new());
    paa.write(&mut out, None).unwrap();
    let bytes = out.into_inner();
    assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0xff01);
    assert_eq!(tag(&bytes, b"GGATCGVA"), Some([0, 0, 255, 255]));
    assert_eq!(tag(&bytes, b"GGATCXAM"), Some([255; 4]));
    assert_eq!(tag(&bytes, b"GGATGALF"), None);
}

#[test]
fn lzo_dxt5_round_trip() {
    // Wider than 128: the mipmap is LZO compressed. Half transparent: DXT5 with a FLAG tag.
    let pixels: Vec<u8> = (0..256 * 256)
        .flat_map(|i| [(i % 256) as u8, (i / 256) as u8, 64, 128])
        .collect();
    let mut paa = Paa::from_image(256, 256, pixels.clone());
    let mut out = Cursor::new(Vec::new());
    paa.write(&mut out, None).unwrap();
    let bytes = out.into_inner();
    assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0xff05);
    assert_eq!(tag(&bytes, b"GGATGALF"), Some([1, 255, 255, 255]));

    let read = Paa::from_reader(&mut Cursor::new(&bytes), None).unwrap();
    assert_eq!(read.magic_number, PaaType::DXT5);
    let first = &read.mipmaps[0];
    assert_eq!((first.width, first.height), (256, 256));
    // The OFFS tag points at the first mipmap, its width has the LZO flag.
    let offset = u32::from_le_bytes(tag(&bytes, b"GGATSFFO").unwrap()) as usize;
    assert_eq!(
        u16::from_le_bytes([bytes[offset], bytes[offset + 1]]),
        0x8000 | 256
    );
    // BC3 is lossy, the gradient comes back close.
    let worst = first
        .data
        .iter()
        .zip(&pixels)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    assert!(worst <= 8, "{worst}");
    assert!(read.mipmaps.len() > 1);

    let blocks = Paa::from_reader_blocks(&mut Cursor::new(&bytes), Some(&[0])).unwrap();
    assert_eq!(blocks.mipmaps[0].data.len(), 256 * 256);
}

#[test]
fn mipmap_read_from_its_header() {
    let file = small_paa(0xff01);
    let mut cursor = Cursor::new(&file);
    cursor.set_position(4);
    let mut mipmap = Mipmap::new();
    mipmap.read(&mut cursor, PaaType::DXT1).unwrap();
    assert_eq!((mipmap.width, mipmap.height), (4, 4));
    // The 16 bytes are two black BC1 blocks' worth, the first block decodes to 4x4 RGBA.
    assert_eq!(mipmap.data.len(), 4 * 4 * 4);
}
