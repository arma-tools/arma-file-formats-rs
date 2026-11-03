//! Black box tests for the rdb reader and writer.
//!
//! `rdb_probes/probe.rdb` is what Workbench wrote for the probe addon of
//! `rdb_probes/make_rdb_probe.sh`, see `rdb_probes/README.md`. `rdb_scan_dir_test` reads the
//! game's files.

use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use arma_file_formats::{
    enfusion::{
        pak::PakTimestamp,
        rdb::{RDB_VERSION, Rdb, RdbEntry, RdbMeta},
        resource::ResourceRef,
    },
    errors::{AffError, RdbError},
};

const DIR: u16 = RdbEntry::UNKNOWN_04 | RdbEntry::DIRECTORY;
const FILE: u16 = RdbEntry::UNKNOWN_04;
const META: u16 = RdbEntry::META;

fn read(bytes: &[u8]) -> Result<Rdb, AffError> {
    Rdb::from(&mut Cursor::new(bytes))
}

fn write(rdb: &Rdb) -> Vec<u8> {
    let mut out = Vec::new();
    rdb.write(&mut out).unwrap();
    out
}

fn rdb_error(result: Result<impl std::fmt::Debug, AffError>) -> RdbError {
    match result {
        Err(AffError::RdbError(e)) => e,
        other => panic!("expected an RdbError, got {other:?}"),
    }
}

fn string(out: &mut Vec<u8>, text: &str) {
    out.extend(((text.len() + 1) as u32).to_le_bytes());
    out.extend(text.as_bytes());
    out.push(0);
}

/// A root, a directory and a tagged file with a meta, built by hand.
fn bytes() -> Vec<u8> {
    let mut body = Vec::new();
    body.extend(7u32.to_le_bytes());
    body.extend(0u64.to_le_bytes());
    body.extend(1u32.to_le_bytes());
    string(&mut body, "tag");
    body.extend(3u32.to_le_bytes());
    for (path, flags, tags, guid, meta) in [
        ("", DIR, 0u32, 0u64, None),
        ("Dir", DIR, 0, 0x1234, None),
        (
            "Dir/File.conf",
            FILE | META,
            1,
            0xABCD,
            Some((0x6842_30C5u32, 300u32)),
        ),
    ] {
        string(&mut body, path);
        body.extend(flags.to_le_bytes());
        body.extend(tags.to_le_bytes());
        body.extend(guid.to_le_bytes());
        if let Some((time, size)) = meta {
            body.extend(time.to_le_bytes());
            body.extend(size.to_le_bytes());
        }
    }
    let mut out = b"FORM".to_vec();
    out.extend(((body.len() + 4) as u32).to_be_bytes());
    out.extend(b"RDBC");
    out.extend(body);
    let len = out.len() as u64;
    out[16..24].copy_from_slice(&len.to_le_bytes());
    out
}

fn expected() -> Rdb {
    Rdb {
        version: RDB_VERSION,
        tags: vec!["tag".into()],
        entries: vec![
            RdbEntry {
                path: String::new(),
                flags: DIR,
                tags: 0,
                guid: 0,
                meta: None,
            },
            RdbEntry {
                path: "Dir".into(),
                flags: DIR,
                tags: 0,
                guid: 0x1234,
                meta: None,
            },
            RdbEntry {
                path: "Dir/File.conf".into(),
                flags: FILE | META,
                tags: 1,
                guid: 0xABCD,
                meta: Some(RdbMeta {
                    modified: PakTimestamp(0x6842_30C5),
                    size: 300,
                }),
            },
        ],
    }
}

#[test]
fn rdb_reads_and_writes_test() {
    let bytes = bytes();
    let rdb = read(&bytes).unwrap();
    assert_eq!(rdb, expected());
    assert_eq!(write(&rdb), bytes);
}

#[test]
fn rdb_lookup_test() {
    let rdb = expected();
    let file = &rdb.entries[2];
    assert_eq!(rdb.find_guid(0xABCD), Some(file));
    assert_eq!(rdb.find_guid(0x9999), None);
    // The root has GUID 0, which means none.
    assert_eq!(rdb.find_guid(0), None);
    assert_eq!(rdb.find_path("dir\\FILE.conf"), Some(file));
    assert_eq!(rdb.find_path("/Dir/"), Some(&rdb.entries[1]));
    assert_eq!(rdb.find_path(""), Some(&rdb.entries[0]));
    assert_eq!(rdb.find_path("Dir/Other.conf"), None);

    // By GUID if there is one, the path is only a hint.
    let moved = ResourceRef::parse("{000000000000ABCD}Old/Place.conf").unwrap();
    assert_eq!(rdb.resolve(&moved), Some(file));
    let path_only = ResourceRef::parse("Dir/File.conf").unwrap();
    assert_eq!(rdb.resolve(&path_only), Some(file));
    let unknown = ResourceRef::parse("{0000000000009999}Dir/File.conf").unwrap();
    assert_eq!(rdb.resolve(&unknown), None);

    assert_eq!(
        file.resource_ref().to_string(),
        "{000000000000ABCD}Dir/File.conf"
    );
    assert_eq!(rdb.entries[0].resource_ref().guid, None);
    assert_eq!(file.name(), "File.conf");
    assert_eq!(rdb.entries[0].name(), "");
    assert!(rdb.entries[1].is_directory());
    assert!(!file.is_directory());

    let map = rdb.guid_map();
    assert_eq!(map.len(), 2);
    assert_eq!(map[&0xABCD], file);

    assert_eq!(rdb.entry_tags(file).collect::<Vec<_>>(), ["tag"]);
    // Bits without a tag are skipped.
    let mut other = file.clone();
    other.tags = 0b110;
    assert_eq!(rdb.entry_tags(&other).count(), 0);
}

#[test]
fn rdb_errors_test() {
    assert!(matches!(read(b"TOPO1234"), Err(AffError::InvalidFileError)));

    let mut form_type = bytes();
    form_type[8..12].copy_from_slice(b"RDBX");
    assert_eq!(
        rdb_error(read(&form_type)),
        RdbError::UnknownFormType("RDBX".into())
    );

    let mut version = bytes();
    version[12] = 6;
    assert_eq!(rdb_error(read(&version)), RdbError::UnsupportedVersion(6));

    let mut size = bytes();
    size[16] += 1;
    let actual = bytes().len() as u64;
    assert_eq!(
        rdb_error(read(&size)),
        RdbError::SizeMismatch {
            header: actual + 1,
            actual
        }
    );

    // The form is cut short.
    let mut cut = bytes();
    cut.truncate(cut.len() - 1);
    assert_eq!(rdb_error(read(&cut)), RdbError::Truncated);

    // A consistent form with one entry too few, or one byte left over.
    let resize = |bytes: &mut Vec<u8>, by: i64| {
        let form = (u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as i64 + by) as u32;
        bytes[4..8].copy_from_slice(&form.to_be_bytes());
        bytes[16..24].copy_from_slice(&(u64::from(form) + 8).to_le_bytes());
    };
    let mut short = bytes();
    short.truncate(short.len() - 4);
    resize(&mut short, -4);
    assert_eq!(rdb_error(read(&short)), RdbError::Truncated);
    let mut trailing = bytes();
    trailing.push(0);
    resize(&mut trailing, 1);
    assert_eq!(rdb_error(read(&trailing)), RdbError::TrailingData);

    // The tag without its NUL, or with a huge length.
    let mut no_nul = bytes();
    no_nul[35] = b'x';
    assert_eq!(rdb_error(read(&no_nul)), RdbError::InvalidString);
    let mut huge = bytes();
    huge[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(rdb_error(read(&huge)), RdbError::Truncated);
    let mut utf8 = bytes();
    utf8[32] = 0xFF;
    assert_eq!(rdb_error(read(&utf8)), RdbError::InvalidString);

    // A huge entry count doesn't allocate before the data runs out.
    let mut count = bytes();
    count[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(rdb_error(read(&count)), RdbError::Truncated);

    let mut rdb = expected();
    rdb.entries[2].meta = None;
    assert_eq!(
        rdb_error(rdb.write(&mut Vec::new())),
        RdbError::MetaFlagMismatch("Dir/File.conf".into())
    );
    let mut rdb = expected();
    rdb.entries[1].flags |= META;
    assert_eq!(
        rdb_error(rdb.write(&mut Vec::new())),
        RdbError::MetaFlagMismatch("Dir".into())
    );
}

/// Workbench's database of the probe addon, see `rdb_probes/README.md`.
#[test]
fn rdb_probe_test() {
    let bytes = std::fs::read("./tests/enfusion/rdb_probes/probe.rdb").unwrap();
    let rdb = read(&bytes).unwrap();
    assert_eq!(write(&rdb), bytes);

    // In the order Workbench came across them, "tagE" (a second string) is dropped.
    assert_eq!(rdb.tags, ["tagA", "tagB", "tagC", "tagD"]);

    let time = |year, month, day, hour, minute, second| {
        PakTimestamp::new(year, month, day, hour, minute, second).unwrap()
    };
    let base = time(2026, 1, 2, 3, 4, 5);
    // Path, flags, tag mask, GUID, meta time and size. Sorted by the lowercased path: `_`
    // before letters, `Registered/` before `Upper`. `ünïcode.conf` is left out.
    type Row<'a> = (&'a str, u16, u32, u64, Option<(PakTimestamp, u32)>);
    #[rustfmt::skip]
    let expected: [Row; 18] = [
        ("", DIR, 0, 0, None),
        ("Configs", DIR, 0, 0xD488_0364_FF9B_45A2, None),
        ("Configs/_under.conf", FILE | META, 0, 0xA000_0000_0000_0009, Some((base, 113))),
        ("Configs/a.conf", FILE | META, 0b0001, 0xA000_0000_0000_0001, Some((time(2026, 12, 31, 23, 59, 58), 121))),
        ("Configs/ab.conf", FILE | META, 0b0011, 0xA000_0000_0000_0003, Some((base, 127))),
        ("Configs/b.conf", FILE | META, 0b0010, 0xA000_0000_0000_0002, Some((base, 121))),
        ("Configs/c.conf", FILE | META, 0b0100, 0xA000_0000_0000_0004, Some((base, 121))),
        ("Configs/cba.conf", FILE | META, 0b0111, 0xA000_0000_0000_0005, Some((base, 133))),
        ("Configs/d.conf", FILE | META, 0b1000, 0xA000_0000_0000_0006, Some((base, 128))),
        ("Configs/nometa.conf", FILE, 0, 0, None),
        ("Configs/none.conf", FILE | META, 0, 0xA000_0000_0000_0007, Some((base, 111))),
        ("Configs/Plain", DIR, 0, 0x9CD4_6E4C_4763_1C90, None),
        ("Configs/Registered", DIR | META, 0, 0xB000_0000_0000_0001, Some((base, 62))),
        ("Configs/Registered/inner.conf", FILE, 0, 0, None),
        ("Configs/Upper.conf", FILE | META, 0, 0xA000_0000_0000_0008, Some((base, 112))),
        ("Language", DIR, 0, 0x4EC3_FBC3_2DB3_F234, None),
        ("Language/strings.st", FILE | META, 0, 0xC000_0000_0000_0001, Some((base, 120))),
        ("RdbProbe.gproj", FILE, 0, 0, None),
    ];
    let actual: Vec<_> = rdb
        .entries
        .iter()
        .map(|e| {
            (
                e.path.as_str(),
                e.flags,
                e.tags,
                e.guid,
                e.meta.map(|m| (m.modified, m.size)),
            )
        })
        .collect();
    assert_eq!(actual, expected);

    let cba = rdb.find_path("configs/CBA.conf").unwrap();
    assert_eq!(
        rdb.entry_tags(cba).collect::<Vec<_>>(),
        ["tagA", "tagB", "tagC"]
    );
}

/// The game's `addons/core/resourceDatabase.rdb` (1.8.0.13).
#[test]
fn rdb_game_core_test() {
    let bytes = std::fs::read("./tests/enfusion/test-data/rdb_in/core.rdb").unwrap();
    let rdb = read(&bytes).unwrap();
    assert_eq!(write(&rdb), bytes);
    assert_eq!(rdb.entries.len(), 1194);

    // The only tag, on the two textures that need a license.
    assert_eq!(rdb.tags, ["license"]);
    let tagged: Vec<_> = rdb
        .entries
        .iter()
        .filter(|e| e.tags != 0)
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(
        tagged,
        [
            "system/textures/milkyway_01_env.edds",
            "system/textures/Moon_CO.edds"
        ]
    );

    // Directory GUIDs are the same in every addon: `Configs` of the probe.
    assert_eq!(
        rdb.find_path("configs").unwrap().guid,
        0xD488_0364_FF9B_45A2
    );

    let schema = rdb.find_path("backend/DSConfigSchema.json").unwrap();
    assert_eq!(schema.guid, 0xBAED_D7B0_D966_31D9);
    assert_eq!(
        schema.meta,
        Some(RdbMeta {
            modified: PakTimestamp::new(2024, 8, 21, 16, 34, 36).unwrap(),
            size: 314,
        })
    );
    assert_eq!(rdb.find_guid(schema.guid), Some(schema));

    // Scripts have no meta.
    let script = rdb.find_path("scripts/Core/constants.c").unwrap();
    assert_eq!((script.flags, script.guid, script.meta), (FILE, 0, None));

    // Directories registered with a meta.
    let registered: Vec<_> = rdb
        .entries
        .iter()
        .filter(|e| e.is_directory() && e.meta.is_some())
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(
        registered,
        [
            "scripts/Core",
            "scripts/GameLib",
            "scripts/WorkbenchCommon",
            "scripts/WorkbenchGameCommon",
            "system/wbdata"
        ]
    );

    let unknown: Vec<_> = rdb
        .entries
        .iter()
        .filter(|e| e.flags & RdbEntry::UNKNOWN_10 != 0)
        .map(|e| (e.path.as_str(), e.flags))
        .collect();
    assert_eq!(
        unknown,
        [("ui/language/default.st", FILE | META | RdbEntry::UNKNOWN_10)]
    );
}

/// Reads every `.rdb` below `AFF_RDB_SCAN_DIR`, e.g. the game's `addons` directory: checks that
/// it writes back identically and the relations the docs describe.
#[test]
fn rdb_scan_dir_test() {
    fn visit(path: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, files);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("rdb"))
            {
                files.push(path);
            }
        }
    }

    let Ok(dir) = std::env::var("AFF_RDB_SCAN_DIR") else {
        return;
    };

    let mut files = Vec::new();
    visit(Path::new(&dir), &mut files);
    assert!(!files.is_empty(), "no rdb files in {dir}");

    for file in files {
        let path = file.display();
        let bytes = std::fs::read(&file).unwrap();
        let rdb = read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(write(&rdb), bytes, "{path}");

        assert!(rdb.tags.len() <= 32, "{path}");
        assert_eq!(rdb.entries[0].path, "", "{path}");
        let lower: Vec<_> = rdb.entries.iter().map(|e| e.path.to_lowercase()).collect();
        assert!(lower.is_sorted(), "{path}");
        for (entry, key) in rdb.entries.iter().zip(&lower).skip(1) {
            let parent = key.rsplit_once('/').map_or("", |(parent, _)| parent);
            let parent = lower.binary_search_by(|p| p.as_str().cmp(parent));
            assert!(
                parent.is_ok_and(|i| rdb.entries[i].is_directory()),
                "{}",
                entry.path
            );
            assert!(entry.flags & RdbEntry::UNKNOWN_04 != 0, "{}", entry.path);
            assert_eq!(entry.tags >> rdb.tags.len(), 0, "{}", entry.path);
            // Files without a meta have no GUID (in fully scanned addons).
            if !entry.is_directory() && entry.meta.is_none() {
                assert_eq!(entry.guid, 0, "{}", entry.path);
            }
        }

        let dirs = rdb.entries.iter().filter(|e| e.is_directory()).count();
        let metas = rdb.entries.iter().filter(|e| e.meta.is_some()).count();
        println!(
            "{path}: {} entries, {dirs} directories, {metas} with meta, tags {:?}",
            rdb.entries.len(),
            rdb.tags
        );
    }
}
