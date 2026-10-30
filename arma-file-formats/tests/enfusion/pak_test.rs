//! Black box tests for the pak reader and writer.
//!
//! `pak_probes/probe.pak` was packed by Arma Reforger Workbench from the sources written by
//! `pak_probes/make_probe.sh`, so the expected contents and metadata are known.

use std::{
    fmt::Write as _,
    fs::File,
    io::{self, BufReader, Cursor, Read, Seek},
    path::{Path, PathBuf},
};

use arma_file_formats::{
    enfusion::pak::{Pak, PakCompression, PakReader, PakSource, PakTimestamp, PakWriter},
    errors::{AffError, PakError},
};

const PROBE_PATH: &str = "./tests/enfusion/pak_probes/probe.pak";

/// Offset of the first file: `FORM` header, `HEAD` chunk and `DATA` chunk header.
const DATA_OFFSET: u32 = 56;

struct Expected {
    path: &'static str,
    data: Vec<u8>,
    timestamp: &'static str,
    compressed: bool,
}

fn big_script() -> Vec<u8> {
    let mut script = String::new();
    for i in 1..=3000 {
        writeln!(
            script,
            "// line {i} padding padding padding padding padding"
        )
        .unwrap();
    }
    script.into_bytes()
}

/// The probe files in the order Workbench writes them: sorted by ASCII uppercased name per
/// directory, so `_` sorts after letters and non-ASCII names come last.
fn expected() -> Vec<Expected> {
    let conf = |path, data: &str| Expected {
        path,
        data: data.as_bytes().to_vec(),
        timestamp: "2026-01-01 00:00:00",
        compressed: false,
    };
    vec![
        conf("Configs/alpha.conf", "Alpha {\n}\n"),
        conf("Configs/Beta/beta.conf", "Beta {\n}\n"),
        conf("Configs/empty.conf", ""),
        conf("Configs/Zeta.conf", "Zeta {\n}\n"),
        conf("Configs/_under.conf", "Under {\n}\n"),
        conf("Configs/ünïcode.conf", "Unicode {\n}\n"),
        Expected {
            path: "Scripts/Game/Deep/Nested/Dir/PP_Big.c",
            data: big_script(),
            timestamp: "2063-12-31 23:59:58",
            compressed: true,
        },
        Expected {
            path: "Scripts/Game/Deep/PP_Tiny.c",
            data: b"class PP_Tiny {}\n".to_vec(),
            timestamp: "2001-02-03 04:05:06",
            compressed: false,
        },
        Expected {
            path: "Scripts/Game/PP_Hello.c",
            data: b"class PP_Hello\n{\n\tvoid Hi() { Print(\"hello\"); }\n}\n".to_vec(),
            timestamp: "2026-10-29 20:51:36",
            compressed: false,
        },
    ]
}

fn assert_matches_expected<R: Read + Seek>(reader: &PakReader<R>) {
    let expected = expected();
    let entries = &reader.pak.entries;
    assert_eq!(
        entries.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
        expected.iter().map(|e| e.path).collect::<Vec<_>>()
    );

    for (entry, expected) in entries.iter().zip(&expected) {
        assert_eq!(
            entry.timestamp.to_string(),
            expected.timestamp,
            "{}",
            entry.path
        );
        assert_eq!(entry.size as usize, expected.data.len(), "{}", entry.path);
        assert_eq!(
            matches!(entry.compression, PakCompression::Zlib { .. }),
            expected.compressed,
            "{}",
            entry.path
        );
        assert_eq!(
            reader.read_entry(entry).unwrap(),
            expected.data,
            "{}",
            entry.path
        );
    }
}

fn pak_error<T: std::fmt::Debug>(result: Result<T, AffError>) -> PakError {
    match result {
        Err(AffError::PakError(e)) => e,
        other => panic!("expected a pak error, got {other:?}"),
    }
}

#[test]
fn pak_probe_entries_test() {
    let reader = PakReader::from_path(PROBE_PATH).unwrap();
    assert_matches_expected(&reader);

    let big = reader
        .pak
        .entry("Scripts/Game/Deep/Nested/Dir/PP_Big.c")
        .unwrap();
    assert_eq!(big.compression, PakCompression::Zlib { level: 6 });
    assert!(big.compressed_size < big.size);
}

#[test]
fn pak_probe_layout_test() {
    let pak = Pak::from_path(PROBE_PATH).unwrap();

    // Data is contiguous in tree order, an empty file shares the offset of the next file.
    let mut offset = DATA_OFFSET;
    for entry in &pak.entries {
        assert_eq!(entry.offset, offset, "{}", entry.path);
        if entry.compression == PakCompression::Stored {
            assert_eq!(entry.compressed_size, entry.size, "{}", entry.path);
        }
        offset += entry.compressed_size;
    }
    let empty = pak.entry("Configs/empty.conf").unwrap();
    assert_eq!(empty.offset, pak.entry("Configs/Zeta.conf").unwrap().offset);
}

#[test]
fn pak_lookup_test() {
    let reader = PakReader::from_path(PROBE_PATH).unwrap();
    assert_eq!(
        reader.read("\\scripts\\GAME\\pp_hello.c").unwrap().unwrap(),
        expected()[8].data
    );
    assert!(reader.read("Scripts/Game").unwrap().is_none());
    assert!(reader.read("Scripts/Game/missing.c").unwrap().is_none());
    assert!(reader.open("Scripts/Game/missing.c").unwrap().is_none());
}

#[test]
fn pak_open_interleaved_test() {
    let reader = PakReader::from_path(PROBE_PATH).unwrap();
    let mut streams = reader
        .pak
        .entries
        .iter()
        .map(|entry| reader.open_entry(entry).unwrap())
        .collect::<Vec<_>>();

    // Every entry is open at once and read a few bytes at a time, round robin.
    let mut contents = vec![Vec::new(); streams.len()];
    loop {
        let mut progress = false;
        for (stream, content) in streams.iter_mut().zip(&mut contents) {
            let mut buf = [0; 7];
            let n = stream.read(&mut buf).unwrap();
            content.extend_from_slice(&buf[..n]);
            progress |= n > 0;
        }
        if !progress {
            break;
        }
    }

    let expected = expected().into_iter().map(|e| e.data).collect::<Vec<_>>();
    assert!(contents == expected, "interleaved reads differ");
}

#[test]
fn pak_size_mismatch_test() {
    let reader = PakReader::from_path(PROBE_PATH).unwrap();
    for path in [
        "Configs/alpha.conf",
        "Scripts/Game/Deep/Nested/Dir/PP_Big.c",
    ] {
        let entry = reader.pak.entry(path).unwrap();
        for size in [entry.size - 1, entry.size + 1] {
            let mut entry = entry.clone();
            entry.size = size;
            match pak_error(reader.read_entry(&entry)) {
                PakError::SizeMismatch { expected, .. } => {
                    assert_eq!(expected, u64::from(size), "{path}");
                }
                other => panic!("{path}: expected a size mismatch, got {other:?}"),
            }
        }
    }
}

#[test]
fn pak_raw_round_trip_test() {
    let original = std::fs::read(PROBE_PATH).unwrap();
    let reader = PakReader::from_stream(Cursor::new(&original)).unwrap();

    // Added in reverse order to check that the writer sorts like Workbench. The data is streamed
    // from the source pak while writing.
    let mut writer = PakWriter::new();
    for entry in reader.pak.entries.iter().rev() {
        let raw = reader.open_entry_raw(entry).unwrap();
        writer.add_raw(entry, PakSource::reader(raw)).unwrap();
    }

    let mut written = Cursor::new(Vec::new());
    writer.write(&mut written).unwrap();
    assert!(
        written.into_inner() == original,
        "raw round trip is not byte identical"
    );
}

#[test]
fn pak_write_test() {
    let expected = expected();
    let mut writer = PakWriter::new();
    for expected in expected.iter().rev() {
        let timestamp = parse_timestamp(expected.timestamp);
        writer
            .add_file(expected.path, &expected.data, timestamp)
            .unwrap();
    }

    let mut written = Cursor::new(Vec::new());
    writer.write(&mut written).unwrap();

    // Same entries and compression choices as Workbench. Compressed bytes may differ since the
    // zlib implementations differ.
    written.set_position(0);
    let reader = PakReader::from_stream(written).unwrap();
    assert_matches_expected(&reader);
}

#[test]
fn pak_write_compression_test() {
    let data = big_script();
    let timestamp = PakTimestamp::new(2030, 6, 15, 12, 30, 45).unwrap();

    let mut writer = PakWriter::new();
    writer
        .add_file_with_compression("stored.c", &data, timestamp, PakCompression::Stored)
        .unwrap();
    writer
        .add_file_with_compression(
            "zlib9.c",
            &data,
            timestamp,
            PakCompression::Zlib { level: 9 },
        )
        .unwrap();
    writer
        .add_file_with_compression("empty.c", b"", timestamp, PakCompression::Zlib { level: 1 })
        .unwrap();

    let mut written = Cursor::new(Vec::new());
    writer.write(&mut written).unwrap();
    written.set_position(0);
    let reader = PakReader::from_stream(written).unwrap();

    let stored = reader.pak.entry("stored.c").unwrap();
    assert_eq!(stored.compression, PakCompression::Stored);
    assert_eq!(stored.compressed_size as usize, data.len());

    let zlib = reader.pak.entry("zlib9.c").unwrap();
    assert_eq!(zlib.compression, PakCompression::Zlib { level: 9 });
    assert!((zlib.compressed_size as usize) < data.len());

    for entry in [stored, zlib] {
        assert_eq!(entry.timestamp, timestamp);
        assert_eq!(reader.read_entry(entry).unwrap(), data);
    }
    assert_eq!(reader.read("empty.c").unwrap().unwrap(), Vec::<u8>::new());
}

#[test]
fn pak_write_sources_test() {
    let probe = std::fs::read(PROBE_PATH).unwrap();
    let script = big_script();
    let timestamp = PakTimestamp::new(2030, 6, 15, 12, 30, 45).unwrap();
    let zlib = PakCompression::Zlib { level: 1 };

    let mut writer = PakWriter::new();
    writer
        .add_file("path_auto.pak", Path::new(PROBE_PATH), timestamp)
        .unwrap();
    writer
        .add_file_with_compression("path_zlib.pak", PathBuf::from(PROBE_PATH), timestamp, zlib)
        .unwrap();
    writer
        .add_file(
            "reader_auto.c",
            PakSource::reader(script.as_slice()),
            timestamp,
        )
        .unwrap();
    writer
        .add_file_with_compression(
            "reader_stored.c",
            PakSource::reader(script.as_slice()),
            timestamp,
            PakCompression::Stored,
        )
        .unwrap();
    writer
        .add_file("owned.c", script.clone(), timestamp)
        .unwrap();

    let mut written = Cursor::new(Vec::new());
    writer.write(&mut written).unwrap();
    written.set_position(0);
    let reader = PakReader::from_stream(written).unwrap();

    for (path, data, compression) in [
        ("path_auto.pak", &probe, PakCompression::Zlib { level: 6 }),
        ("path_zlib.pak", &probe, zlib),
        ("reader_auto.c", &script, PakCompression::Zlib { level: 6 }),
        ("reader_stored.c", &script, PakCompression::Stored),
        ("owned.c", &script, PakCompression::Zlib { level: 6 }),
    ] {
        let entry = reader.pak.entry(path).unwrap();
        assert_eq!(entry.compression, compression, "{path}");
        assert_eq!(&reader.read_entry(entry).unwrap(), data, "{path}");
    }
}

#[test]
fn pak_write_source_errors_test() {
    let mut writer = PakWriter::new();
    writer
        .add_file(
            "missing.c",
            Path::new("missing/missing.c"),
            PakTimestamp::default(),
        )
        .unwrap();
    match writer.write(&mut Cursor::new(Vec::new())) {
        Err(AffError::IOError(e)) => assert_eq!(e.kind(), io::ErrorKind::NotFound),
        other => panic!("expected an io error, got {other:?}"),
    }

    // Raw data that does not match the entry it is added for.
    let reader = PakReader::from_path(PROBE_PATH).unwrap();
    let entry = reader.pak.entry("Configs/alpha.conf").unwrap();
    let mut writer = PakWriter::new();
    writer.add_raw(entry, b"short").unwrap();
    assert_eq!(
        pak_error(writer.write(&mut Cursor::new(Vec::new()))),
        PakError::SizeMismatch {
            path: entry.path.clone(),
            expected: u64::from(entry.compressed_size),
            actual: 5,
        }
    );
}

#[test]
fn pak_write_invalid_paths_test() {
    let timestamp = PakTimestamp::default();
    let write = |paths: &[&str]| -> Result<(), AffError> {
        let mut writer = PakWriter::new();
        for path in paths {
            writer.add_file(path, b"data", timestamp)?;
        }
        writer.write(&mut Cursor::new(Vec::new()))
    };

    assert!(write(&["a/b.c", "a/B/c.c", "\\A\\d.c"]).is_ok());
    assert_eq!(
        pak_error(write(&["a/b.c", "A/B.C"])),
        PakError::DuplicatePath("A/B.C".into())
    );
    assert_eq!(
        pak_error(write(&["a/b", "a/b/c.c"])),
        PakError::DuplicatePath("a/b/c.c".into())
    );
    assert_eq!(
        pak_error(write(&["a/b/c.c", "a/b"])),
        PakError::DuplicatePath("a/b".into())
    );
    assert_eq!(
        pak_error(write(&["a//b.c"])),
        PakError::InvalidPath("a//b.c".into())
    );
    assert_eq!(
        pak_error(write(&["a/"])),
        PakError::InvalidPath("a/".into())
    );
    assert_eq!(
        pak_error(write(&[""])),
        PakError::InvalidPath(String::new())
    );
    let long = "x".repeat(256);
    assert_eq!(
        pak_error(write(&[long.as_str()])),
        PakError::InvalidPath(long.clone())
    );
    assert!(write(&[&long[..255]]).is_ok());
}

#[test]
fn pak_checksum_test() {
    let mut data = std::fs::read(PROBE_PATH).unwrap();
    // Last byte of the FILE chunk: high byte of the timestamp of the last entry.
    *data.last_mut().unwrap() ^= 1;
    match Pak::from_stream(&mut Cursor::new(data)) {
        Err(AffError::PakError(PakError::ChecksumMismatch)) => {}
        other => panic!("expected a checksum mismatch, got {other:?}"),
    }
}

#[test]
fn pak_timestamp_test() {
    let timestamp = PakTimestamp(0x6ABB_4CE4);
    assert_eq!(timestamp.to_string(), "2026-10-29 20:51:36");
    assert_eq!(timestamp.to_unix_secs(), 1_793_307_096);
    assert_eq!(PakTimestamp::from_unix_secs(1_793_307_096), Some(timestamp));
    assert_eq!(PakTimestamp::new(2026, 10, 29, 20, 51, 36), Some(timestamp));

    for (secs, text) in [
        (946_684_800, "2000-01-01 00:00:00"),
        (951_782_400, "2000-02-29 00:00:00"),
        (2_966_371_199, "2063-12-31 23:59:59"),
    ] {
        let timestamp = PakTimestamp::from_unix_secs(secs).unwrap();
        assert_eq!(timestamp.to_string(), text);
        assert_eq!(timestamp.to_unix_secs(), secs);
    }

    assert_eq!(PakTimestamp::from_unix_secs(946_684_799), None);
    assert_eq!(PakTimestamp::from_unix_secs(2_966_371_200), None);
    assert_eq!(PakTimestamp::new(2026, 13, 1, 0, 0, 0), None);
    assert_eq!(PakTimestamp::new(2026, 1, 1, 24, 0, 0), None);
}

fn parse_timestamp(text: &str) -> PakTimestamp {
    let mut fields = text.split(['-', ' ', ':']);
    let mut next = || fields.next().unwrap();
    PakTimestamp::new(
        next().parse().unwrap(),
        next().parse().unwrap(),
        next().parse().unwrap(),
        next().parse().unwrap(),
        next().parse().unwrap(),
        next().parse().unwrap(),
    )
    .unwrap()
}

/// Repacks the pak at `AFF_PAK_REPACK` into the temp directory by streaming the raw entries and
/// checks that the result is byte identical.
#[test]
fn pak_repack_test() {
    let Ok(source) = std::env::var("AFF_PAK_REPACK") else {
        return;
    };
    let target = std::env::temp_dir().join("aff_pak_repack_test.pak");

    let reader = PakReader::from_stream(BufReader::new(File::open(&source).unwrap())).unwrap();
    let mut writer = PakWriter::new();
    for entry in &reader.pak.entries {
        let raw = reader.open_entry_raw(entry).unwrap();
        writer.add_raw(entry, PakSource::reader(raw)).unwrap();
    }
    let mut out = io::BufWriter::new(File::create(&target).unwrap());
    writer.write(&mut out).unwrap();
    drop(out);

    let identical = files_identical(Path::new(&source), &target);
    std::fs::remove_file(&target).unwrap();
    assert!(identical, "repack of {source} is not byte identical");
    println!("{source}: {} entries", reader.pak.entries.len());
}

fn files_identical(a: &Path, b: &Path) -> bool {
    if std::fs::metadata(a).unwrap().len() != std::fs::metadata(b).unwrap().len() {
        return false;
    }
    let mut a = BufReader::new(File::open(a).unwrap());
    let mut b = BufReader::new(File::open(b).unwrap());
    let (mut buf_a, mut buf_b) = (vec![0; 1 << 16], vec![0; 1 << 16]);
    loop {
        let n = a.read(&mut buf_a).unwrap();
        if n == 0 {
            return true;
        }
        b.read_exact(&mut buf_b[..n]).unwrap();
        if buf_a[..n] != buf_b[..n] {
            return false;
        }
    }
}

/// Reads every `.pak` below `AFF_PAK_SCAN_DIR`, e.g. the game's `addons` directory: checks the
/// tree order and data layout and decompresses every zlib entry.
#[test]
fn pak_scan_dir_test() {
    fn visit(path: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, files);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pak"))
            {
                files.push(path);
            }
        }
    }

    let Ok(dir) = std::env::var("AFF_PAK_SCAN_DIR") else {
        return;
    };

    let mut files = Vec::new();
    visit(Path::new(&dir), &mut files);
    assert!(!files.is_empty(), "no pak files in {dir}");

    let sort_key = |path: &str| {
        path.split('/')
            .map(str::to_ascii_uppercase)
            .collect::<Vec<_>>()
    };

    for path in files {
        let reader = PakReader::from_stream(BufReader::new(File::open(&path).unwrap()))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let entries = &reader.pak.entries;

        let mut offset = DATA_OFFSET;
        for (i, entry) in entries.iter().enumerate() {
            if i > 0 {
                assert!(
                    sort_key(&entries[i - 1].path) < sort_key(&entry.path),
                    "{}: {} not sorted",
                    path.display(),
                    entry.path
                );
            }
            assert_eq!(entry.offset, offset, "{}: {}", path.display(), entry.path);
            offset += entry.compressed_size;

            if matches!(entry.compression, PakCompression::Zlib { .. }) {
                let mut stream = reader.open_entry(entry).unwrap();
                io::copy(&mut stream, &mut io::sink())
                    .unwrap_or_else(|e| panic!("{}: {}: {e}", path.display(), entry.path));
            }
        }
        println!("{}: {} entries", path.display(), entries.len());
    }
}
