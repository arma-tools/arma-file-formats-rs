use std::{fs::File, io::BufReader};

use arma_file_formats::{core::types::PixelType, enfusion::edds::Edds};
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/enfusion/test-data/edds_in/";

const OUTPUT_PATH_PREFIX: &str = "./tests/enfusion/test-data/edds_out/";

fn export_mipmaps(edds: &Edds, filename: &str, color_type: image::ColorType) {
    for (i, mipmap) in edds.mipmaps.iter().enumerate() {
        image::save_buffer(
            format!("{}{}.out.{}.png", OUTPUT_PATH_PREFIX, filename, i),
            &mipmap.data,
            mipmap.width as u32,
            mipmap.height as u32,
            color_type,
        )
        .unwrap();
    }
}

#[test]
#[serial]
fn edds_bc4_test() {
    let file = File::open(format!("{}prop_bc4.edds", INPUT_PATH_PREFIX)).unwrap();
    let edds = Edds::from(&mut BufReader::new(file)).unwrap();

    assert_eq!(edds.mipmaps.len(), 12_usize);
    assert_eq!(edds.mipmaps[11].width, 1024_usize);
    assert_eq!(edds.mipmaps[11].height, 2048_usize);
    assert_eq!(edds.mipmaps[11].data.len(), 2097152_usize);

    assert_eq!(edds.pixel_type, PixelType::Gray);

    export_mipmaps(&edds, "prop_bc4", image::ColorType::L8);
}

#[test]
#[serial]
fn edds_bc7_test() {
    let file = File::open(format!("{}car_bc7.edds", INPUT_PATH_PREFIX)).unwrap();
    let edds = Edds::from(&mut BufReader::new(file)).unwrap();

    assert_eq!(edds.mipmaps.len(), 12_usize);
    assert_eq!(edds.mipmaps[11].width, 2048_usize);
    assert_eq!(edds.mipmaps[11].height, 2048_usize);
    assert_eq!(edds.mipmaps[11].data.len(), 16777216_usize);
    assert_eq!(edds.pixel_type, PixelType::Rgba);

    export_mipmaps(&edds, "car_bc7", image::ColorType::Rgba8);
}

#[test]
#[serial]
fn edds_rgba_test() {
    let file = File::open(format!("{}uaz_rgba.edds", INPUT_PATH_PREFIX)).unwrap();
    let edds = Edds::from(&mut BufReader::new(file)).unwrap();

    assert_eq!(edds.mipmaps.len(), 10_usize);
    assert_eq!(edds.mipmaps[9].width, 800_usize);
    assert_eq!(edds.mipmaps[9].height, 600_usize);
    assert_eq!(edds.mipmaps[9].data.len(), 1920000_usize);
    assert_eq!(edds.pixel_type, PixelType::Rgba);

    export_mipmaps(&edds, "uaz_rgba", image::ColorType::Rgba8);
}

#[test]
#[serial]
fn edds_non_dx10_header_test() {
    let file = File::open(format!("{}optic.edds", INPUT_PATH_PREFIX)).unwrap();
    let edds = Edds::from(&mut BufReader::new(file)).unwrap();

    assert_eq!(edds.mipmaps.len(), 11_usize);
    assert_eq!(edds.mipmaps[10].width, 1024_usize);
    assert_eq!(edds.mipmaps[10].height, 1024_usize);
    assert_eq!(edds.mipmaps[10].data.len(), 4194304_usize);
    assert_eq!(edds.pixel_type, PixelType::Rgba);

    export_mipmaps(&edds, "optic", image::ColorType::Rgba8);
}

#[test]
#[serial]
fn eden_1337_layer_test() {
    let file = File::open(format!("{}Eden_1337_layer.edds", INPUT_PATH_PREFIX)).unwrap();
    let edds = Edds::from(&mut BufReader::new(file)).unwrap();

    assert_eq!(edds.mipmaps.len(), 9_usize);
    assert_eq!(edds.mipmaps[8].width, 256_usize);
    assert_eq!(edds.mipmaps[8].height, 256_usize);
    assert_eq!(edds.mipmaps[8].data.len(), 262144_usize);
    assert_eq!(edds.pixel_type, PixelType::Rgba);

    export_mipmaps(&edds, "Eden_1337_layer_test", image::ColorType::Rgba8);
}

#[test]
#[serial]
fn eden_1337_normal_test() {
    let file = File::open(format!("{}Eden_1337_normal.edds", INPUT_PATH_PREFIX)).unwrap();
    let edds = Edds::from(&mut BufReader::new(file)).unwrap();

    assert_eq!(edds.mipmaps.len(), 9_usize);
    assert_eq!(edds.mipmaps[8].width, 256_usize);
    assert_eq!(edds.mipmaps[8].height, 256_usize);
    assert_eq!(edds.mipmaps[8].data.len(), 262144_usize);
    assert_eq!(edds.pixel_type, PixelType::Rgba);

    export_mipmaps(&edds, "Eden_1337_normal_test", image::ColorType::Rgba8);
}

#[test]
#[serial]
fn eden_1337_supertexture_test() {
    let file = File::open(format!("{}Eden_1337_supertexture.edds", INPUT_PATH_PREFIX)).unwrap();
    let edds = Edds::from(&mut BufReader::new(file)).unwrap();

    assert_eq!(edds.mipmaps.len(), 9_usize);
    assert_eq!(edds.mipmaps[8].width, 256_usize);
    assert_eq!(edds.mipmaps[8].height, 256_usize);
    assert_eq!(edds.mipmaps[8].data.len(), 262144_usize);
    assert_eq!(edds.pixel_type, PixelType::Rgba);

    export_mipmaps(
        &edds,
        "Eden_1337_supertexture_test",
        image::ColorType::Rgba8,
    );
}
