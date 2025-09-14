use arma_file_formats::core::check_for_magic_and_decompress_lzss;
use serial_test::serial;
use std::fs::{self, File};

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/util_in/";
const OUTPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/util_out/";

#[test]
#[serial]
fn lzss_test_shp() {
    let mut file = File::open(format!("{}roads_lzss.shp", INPUT_PATH_PREFIX)).unwrap();

    let res = check_for_magic_and_decompress_lzss(&mut file, &[0, 0, 0x27, 0x0A]).unwrap();

    let data = res.unwrap();

    fs::write(format!("{}roads_lzss_uncom.shp", OUTPUT_PATH_PREFIX), data).unwrap();
}
