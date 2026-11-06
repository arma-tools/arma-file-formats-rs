use std::{
    fs::{self, File},
    io::BufReader,
};

use arma_file_formats::real_virtuality::{
    pbo::{Pbo, PboReader},
    sign::{PrivateKey, PublicKey, SignVersion, Signature},
};
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/pbo_in/";
const OUTPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/pbo_out/";

#[test]
#[serial]
fn test_fow() {
    let pbo = Pbo::from_path(format!("{}fow_functions.pbo", INPUT_PATH_PREFIX)).unwrap();
    dbg!(pbo.entries);
}

#[test]
#[serial]
fn has_entry() {
    let pbo = Pbo::from_path(format!("{}grad_adminMessages.pbo", INPUT_PATH_PREFIX)).unwrap();
    assert!(pbo.has_entry("x\\grad\\addons\\adminMessages\\gui\\defines.hpp"));
}

#[test]
#[serial]
fn pbo() {
    let file = File::open(format!("{}grad_adminMessages.pbo", INPUT_PATH_PREFIX)).unwrap();
    let mut buffer = BufReader::new(file);
    let pbo = Pbo::from_stream(&mut buffer).unwrap();

    let files: Vec<String> = pbo.entries.values().map(|e| e.filename.clone()).collect();
    dbg!("{:?}", files);
}

#[test]
#[serial]
fn pbo_lazy() {
    let file = File::open(format!("{}grad_adminMessages.pbo", INPUT_PATH_PREFIX)).unwrap();
    let mut buffer = BufReader::new(file);
    let mut pbo = PboReader::from_stream(&mut buffer).unwrap();

    let file2 = pbo.get_entry("stringtable.xml").unwrap().unwrap();

    fs::write(format!("{}stringtable.xml", OUTPUT_PATH_PREFIX), file2.data).unwrap();

    assert!(
        pbo.extract_single_file(
            "gui\\defines.hpp",
            &format!("{}extracttest/", OUTPUT_PATH_PREFIX),
            false
        )
        .is_ok()
    );
}

#[test]
#[serial]
fn verify_sig() {
    let mut pub_key_file = File::open(format!("{}AFF_TEST_KEY.bikey", INPUT_PATH_PREFIX)).unwrap();
    let pub_key = PublicKey::from_stream(&mut pub_key_file).unwrap();

    let mut sig_file = File::open(format!(
        "{}grad_adminMessages.pbo.AFF_TEST_KEY.bisign",
        INPUT_PATH_PREFIX
    ))
    .unwrap();
    let sig = Signature::from_stream(&mut sig_file).unwrap();

    let pbo_file = File::open(format!("{}grad_adminMessages.pbo", INPUT_PATH_PREFIX)).unwrap();
    let mut pbo_buf_read = BufReader::new(pbo_file);
    let pbo = Pbo::from_stream(&mut pbo_buf_read).unwrap();

    assert!(pbo.verify(&pub_key, &sig).is_ok());
}

#[test]
#[serial]
fn read_priv_key() {
    let mut priv_key_file =
        File::open(format!("{}AFF_TEST_KEY.biprivatekey", INPUT_PATH_PREFIX)).unwrap();
    let _ = PrivateKey::from_stream(&mut priv_key_file).unwrap();
}

#[test]
#[serial]
fn sign_test() {
    let pbo = Pbo::from_path(format!("{}grad_adminMessages.pbo", INPUT_PATH_PREFIX)).unwrap();

    let auth = "AFF_TEST_KEY2";

    let mut priv_key = PrivateKey::generate(auth);
    priv_key
        .write_file(format!("{}AFF_TEST_KEY2", OUTPUT_PATH_PREFIX))
        .unwrap();

    let mut pub_key: PublicKey = priv_key.clone().into();
    pub_key
        .write_file(format!("{}AFF_TEST_KEY2", OUTPUT_PATH_PREFIX))
        .unwrap();

    let mut sig = pbo.sign(SignVersion::V3, &priv_key);
    sig.write_file(format!("{}AFF_TEST_KEY2", OUTPUT_PATH_PREFIX))
        .unwrap();

    assert!(pbo.verify(&pub_key, &sig).is_ok());
}

#[test]
#[serial]
fn lingor_compression_test() {
    let pbo = Pbo::from_path(format!("{}lingor_objects.pbo", INPUT_PATH_PREFIX)).unwrap();
    assert_eq!(pbo.entries.get("skala3_3.p3d").unwrap().data.len(), 118923);
}

/// Compressed entries of obfuscated PBOs: data after the LZSS stream and garbage original sizes
/// give the stored bytes back instead of panicking or allocating the garbage size.
#[test]
fn garbage_compressed_entries() {
    // LZSS: a flag byte for 8 literals, 3 of them, the checksum (sum of the bytes).
    let lzss = [0xff, b'a', b'b', b'c', 0x26, 0x01, 0, 0];
    let mut trailing = lzss.to_vec();
    trailing.extend_from_slice(b"junk");

    let mut pbo = vec![0];
    pbo.extend_from_slice(b"sreV");
    pbo.extend_from_slice(&[0; 16]);
    pbo.extend_from_slice(b"prefix\0garbage\0\0");
    for (name, original_size, data) in [
        ("ok.txt", 3, &lzss[..]),
        ("trailing.txt", 3, &trailing[..]),
        ("huge.txt", u32::MAX, &lzss[..]),
    ] {
        pbo.extend_from_slice(name.as_bytes());
        pbo.push(0);
        pbo.extend_from_slice(b"srpC");
        for value in [original_size, 0, 0, data.len() as u32] {
            pbo.extend_from_slice(&value.to_le_bytes());
        }
    }
    pbo.extend_from_slice(&[0; 21]);
    pbo.extend_from_slice(&lzss);
    pbo.extend_from_slice(&trailing);
    pbo.extend_from_slice(&lzss);
    pbo.extend_from_slice(&[0; 21]);

    let mut reader = PboReader::from_stream(std::io::Cursor::new(pbo)).unwrap();
    let data = |reader: &mut PboReader<_>, name| reader.get_entry(name).unwrap().unwrap().data;
    assert_eq!(data(&mut reader, "ok.txt"), b"abc");
    assert_eq!(data(&mut reader, "trailing.txt"), trailing);
    assert_eq!(data(&mut reader, "huge.txt"), lzss);
}
