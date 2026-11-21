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

/// An LZSS stream of only literals: a flag byte of `0xFF` before every 8 bytes, then the sum of
/// the bytes as `i32`.
fn lzss_literals(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in data.chunks(8) {
        out.push(0xFF);
        out.extend(chunk);
    }
    let sum = data
        .iter()
        .fold(0i32, |sum, &b| sum.wrapping_add(i32::from(b)));
    out.extend(sum.to_le_bytes());
    out
}

#[test]
fn lzss_magic_check_and_file_rewrite() {
    use arma_file_formats::core::{
        check_for_magic_and_decompress_lzss_file, decompress_lzss_unk_size,
    };
    use std::io::Cursor;

    let data = b"OPRW and some more text to compress".to_vec();
    let compressed = lzss_literals(&data);
    assert_eq!(
        decompress_lzss_unk_size(&mut Cursor::new(&compressed)).unwrap(),
        data
    );
    assert_eq!(
        check_for_magic_and_decompress_lzss(&mut Cursor::new(&compressed), b"OPRW").unwrap(),
        Some(data.clone())
    );
    assert_eq!(
        check_for_magic_and_decompress_lzss(&mut Cursor::new(&compressed), b"ODOL").unwrap(),
        None
    );

    // A wrong checksum is an error.
    let mut broken = compressed.clone();
    *broken.last_mut().unwrap() ^= 1;
    assert!(decompress_lzss_unk_size(&mut Cursor::new(&broken)).is_err());

    // The file variant replaces the file with the decompressed data.
    let path = std::env::temp_dir().join(format!("aff_lzss_test_{}.bin", std::process::id()));
    fs::write(&path, &compressed).unwrap();
    assert!(check_for_magic_and_decompress_lzss_file(&path, b"OPRW").unwrap());
    assert_eq!(fs::read(&path).unwrap(), data);
    assert!(!check_for_magic_and_decompress_lzss_file(&path, b"OPRW").unwrap());
    fs::remove_file(&path).unwrap();
}

#[test]
fn lzss_compressed_odol_reads_like_the_plain_file() {
    use arma_file_formats::real_virtuality::p3d::ODOL;
    use std::io::Cursor;

    let plain = fs::read("./tests/real_virtuality/test-data/p3d_in/Crash_barrier_F.p3d").unwrap();
    let odol = ODOL::from_stream(&mut Cursor::new(&plain)).unwrap();
    let compressed = ODOL::from_stream(&mut Cursor::new(lzss_literals(&plain))).unwrap();
    // Debug output, some LODs hold NaN floats.
    assert_eq!(format!("{compressed:?}"), format!("{odol:?}"));
}
