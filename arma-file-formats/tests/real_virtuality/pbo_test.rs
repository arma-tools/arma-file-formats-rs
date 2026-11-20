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

/// A PBO in memory with these files.
fn memory_pbo(prefix: &str, files: &[(&str, &[u8])]) -> Pbo {
    let mut pbo = Pbo::new();
    pbo.properties
        .insert("prefix".to_string(), prefix.to_string());
    for (name, data) in files {
        let mut entry = arma_file_formats::real_virtuality::pbo::Entry::new();
        entry.filename = (*name).to_string();
        entry.data = data.to_vec();
        pbo.entries.insert((*name).to_string(), entry);
    }
    pbo.hash = vec![7; 20];
    pbo
}

fn with_data(pbo: &Pbo, name: &str, data: &[u8]) -> Pbo {
    let mut changed = pbo.clone();
    changed.entries.get_mut(name).unwrap().data = data.to_vec();
    changed
}

#[test]
fn sign_versions_hash_different_files() {
    let key = PrivateKey::generate("AFF_TEST_HASH");
    let public: PublicKey = key.clone().into();
    let pbo = memory_pbo(
        "x\\test",
        &[
            ("config.bin", b"config"),
            ("fn_test.sqf", b"hint 1;"),
            ("data\\texture_co.paa", b"pixels"),
        ],
    );
    for version in [SignVersion::V2, SignVersion::V3] {
        let sig = pbo.sign(version, &key);
        assert!(pbo.verify(&public, &sig).is_ok(), "{version:?}");

        // Textures are in neither hash.
        let texture = with_data(&pbo, "data\\texture_co.paa", b"other pixels");
        assert!(texture.verify(&public, &sig).is_ok(), "{version:?}");

        // Scripts are in both.
        let script = with_data(&pbo, "fn_test.sqf", b"hint 2;");
        let err = script.verify(&public, &sig).unwrap_err();
        assert_eq!(err.to_string(), "the file contents differ (hash 3)");

        // Binarized configs only in V2.
        let config = with_data(&pbo, "config.bin", b"other config");
        assert_eq!(
            config.verify(&public, &sig).is_ok(),
            version == SignVersion::V3,
            "{version:?}"
        );

        // Hashed in the order of the lower case paths, not as stored.
        let mut reordered = pbo.clone();
        reordered.entries.reverse();
        assert!(reordered.verify(&public, &sig).is_ok(), "{version:?}");

        // The names are in hash 2.
        let mut renamed = pbo.clone();
        let entry = renamed.entries.shift_remove("fn_test.sqf").unwrap();
        renamed.entries.insert("fn_other.sqf".to_string(), entry);
        let err = renamed.verify(&public, &sig).unwrap_err();
        assert_eq!(err.to_string(), "the file names differ (hash 2)");

        // And the checksum of the file in hash 1.
        let mut rehashed = pbo.clone();
        rehashed.hash = vec![8; 20];
        let err = rehashed.verify(&public, &sig).unwrap_err();
        assert_eq!(err.to_string(), "the PBO checksum differs (hash 1)");
    }
}

#[test]
fn sign_without_hashed_files_and_prefix_with_backslash() {
    let key = PrivateKey::generate("AFF_TEST_EMPTY");
    let public: PublicKey = key.clone().into();
    // Only a texture: the file hash is of "nothing" (V2) or "gnihton" (V3).
    let pbo = memory_pbo("x\\test\\", &[("texture_co.paa", b"pixels")]);
    for version in [SignVersion::V2, SignVersion::V3] {
        let sig = pbo.sign(version, &key);
        assert!(pbo.verify(&public, &sig).is_ok());
        // The prefix ends with a `\` either way.
        let mut without = pbo.clone();
        without
            .properties
            .insert("prefix".to_string(), "x\\test".to_string());
        assert!(without.verify(&public, &sig).is_ok());
    }
}

#[test]
fn verify_needs_the_signing_key() {
    let key = PrivateKey::generate("AFF_TEST_KEY3");
    let public: PublicKey = key.clone().into();
    let pbo = memory_pbo("x\\test", &[("fn_test.sqf", b"hint 1;")]);
    let sig = pbo.sign(SignVersion::V3, &key);

    // Another key with the same authority name.
    let forger = PrivateKey::generate("AFF_TEST_KEY3");
    let forged = pbo.sign(SignVersion::V3, &forger);
    let err = pbo.verify(&public, &forged).unwrap_err();
    assert_eq!(
        err.to_string(),
        "the signature was made with another key of authority AFF_TEST_KEY3"
    );

    // Another authority.
    let other: PublicKey = PrivateKey::generate("AFF_TEST_OTHER").into();
    let err = pbo.verify(&other, &sig).unwrap_err();
    assert_eq!(
        err.to_string(),
        "the signature is from authority AFF_TEST_KEY3, the key from AFF_TEST_OTHER"
    );
}

#[test]
fn pbo_entries_and_errors() {
    use arma_file_formats::{errors::AffError, real_virtuality::pbo::Entry};
    use std::io::Cursor;

    let data = fs::read(format!("{INPUT_PATH_PREFIX}grad_adminMessages.pbo")).unwrap();
    let mut reader = PboReader::from_stream(Cursor::new(&data)).unwrap();
    assert_eq!(reader.get_prefix(), "x\\grad\\addons\\adminmessages\\");
    assert!(reader.get_entry("missing.sqf").unwrap().is_none());
    assert!(matches!(
        reader.extract_single_file("missing.sqf", OUTPUT_PATH_PREFIX, false),
        Err(AffError::PboEntryNotFound(_))
    ));
    // With the full path, the entry's directories are created under the output path.
    let out = format!("{OUTPUT_PATH_PREFIX}extract_full/");
    reader
        .extract_single_file("gui\\defines.hpp", &out, true)
        .unwrap();
    assert!(fs::metadata(format!("{out}gui/defines.hpp")).is_ok());

    // Not a PBO, or the data doesn't end where the headers say.
    assert!(matches!(
        Pbo::from_stream(&mut Cursor::new(vec![1u8; 64])),
        Err(AffError::InvalidFileError)
    ));
    let mut header = vec![0u8];
    header.extend(b"sreV");
    header.extend([1; 16]);
    header.extend([0; 32]);
    assert!(matches!(
        Pbo::from_stream(&mut Cursor::new(header)),
        Err(AffError::InvalidFileError)
    ));
    let mut no_end = data.clone();
    let len = no_end.len();
    no_end[len - 21] = 1;
    assert!(matches!(
        Pbo::from_stream(&mut Cursor::new(no_end)),
        Err(AffError::InvalidFileError)
    ));

    // An entry header round trips (the name is read lower case).
    let mut bytes = b"dir\\file.sqf\0".to_vec();
    for value in [0x7372_7043u32, 100, 0, 1_234_567, 80] {
        bytes.extend(value.to_le_bytes());
    }
    let mut entry = Entry::new();
    entry.read(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(entry.filename, "dir\\file.sqf");
    assert_eq!(entry.mime_type, "Cprs");
    assert_eq!((entry.original_size, entry.timestamp), (100, 1_234_567));
    let mut out = Cursor::new(Vec::new());
    entry.write(&mut out).unwrap();
    assert_eq!(out.into_inner(), bytes);
}
