use std::{fs::File, io::BufReader};

use image::{GenericImageView, ImageBuffer};
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
