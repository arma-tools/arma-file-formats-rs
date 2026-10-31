use std::{fs::File, io::BufReader};

use arma_file_formats::{
    core::types::PixelType,
    enfusion::edds::{Edds, EddsFormat, Mipmap},
};
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

/// Opens `name`, checks the format and mipmap count and returns the largest mipmap.
fn largest_mipmap(name: &str, format: EddsFormat, mipmaps: usize) -> Mipmap {
    let file = File::open(format!("{}{}", INPUT_PATH_PREFIX, name)).unwrap();
    let mut edds = Edds::from(&mut BufReader::new(file)).unwrap();

    assert_eq!(edds.format, format, "{name}");
    assert_eq!(edds.mipmaps.len(), mipmaps, "{name}");
    edds.mipmaps.pop().unwrap()
}

fn byte_sum(data: &[u8]) -> u64 {
    data.iter().map(|&b| u64::from(b)).sum()
}

#[test]
fn edds_legacy_r8_flow_test() {
    // River flow maps: no DX10 header, 8 bit R mask, 4 pixels wide, one `LZ4 ` level.
    for (name, height, mipmaps, start, sum) in [
        (
            "572B18098A717D07_flow.edds",
            512,
            10,
            [4, 18, 19, 6],
            82_312,
        ),
        ("5888C09A875F1300_flow.edds", 108, 7, [0, 3, 3, 14], 19_306),
        ("5D6094935B7ECD09_flow.edds", 320, 9, [0, 0, 0, 0], 58_834),
    ] {
        let mipmap = largest_mipmap(name, EddsFormat::L8, mipmaps);

        assert_eq!((mipmap.width, mipmap.height), (4, height), "{name}");
        assert_eq!(mipmap.data.len(), 4 * height, "{name}");
        assert_eq!(mipmap.data[..4], start, "{name}");
        assert_eq!(byte_sum(&mipmap.data), sum, "{name}");
    }
}

#[test]
fn edds_legacy_abgr_shore_atlas_test() {
    // Shore distance field atlas: no DX10 header, R in the lowest byte, one multi block LZ4 mip.
    let mipmap = largest_mipmap("shoreDFAtlas.edds", EddsFormat::Rgba8 { srgb: false }, 1);

    assert_eq!((mipmap.width, mipmap.height), (2244, 924));
    assert_eq!(mipmap.data.len(), 2244 * 924 * 4);
    assert_eq!(mipmap.data[..8], [255, 255, 241, 184, 255, 255, 234, 198]);
    assert_eq!(byte_sum(&mipmap.data), 1_057_952_939);
}

#[test]
fn edds_bc4_shore_atlas_test() {
    // Shore mask atlases: BC4 at a size that isn't a power of two, a single LZ4 mip.
    for (name, sum) in [
        ("shoreDFNearAtlas.edds", 380_047_033),
        ("shoreFoamMaskAtlas.edds", 1_005_242),
        ("shoreMaskAtlas.edds", 270_742_340),
    ] {
        let mipmap = largest_mipmap(name, EddsFormat::Bc4, 1);

        assert_eq!((mipmap.width, mipmap.height), (2244, 924), "{name}");
        assert_eq!(mipmap.data.len(), 2244 * 924, "{name}");
        assert_eq!(byte_sum(&mipmap.data), sum, "{name}");
    }
}
