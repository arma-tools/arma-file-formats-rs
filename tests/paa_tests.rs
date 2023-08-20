use std::{
    fs::{self, File},
    io::{BufReader, Cursor},
};

use image::ImageBuffer;
use rvff::{
    self,
    paa::{Paa, Tagg},
};
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/test-data/paa_in/";
const OUTPUT_PATH_PREFIX: &str = "./tests/test-data/paa_out/";

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

    let img_buf: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(mm.width.into(), mm.height.into(), mm.data.clone()).unwrap();

    img_buf
        .save(format!("{}ai88_plus.png", OUTPUT_PATH_PREFIX))
        .unwrap();
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
}
