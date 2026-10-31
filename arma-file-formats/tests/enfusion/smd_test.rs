//! Black box tests for the smd reader and writer.
//!
//! `smd_probes/probe.smd` was built by Workbench for a probe world, see `smd_probes/README.md`.
//! Game files can't be committed, the other inputs are assembled by hand from the layout of the
//! game's smds. `smd_scan_dir_test` reads the game's files.

use std::{
    fs::File,
    io::{BufReader, Cursor},
    num::NonZeroU16,
    path::{Path, PathBuf},
};

use arma_file_formats::{
    enfusion::{
        edds::RawEdds,
        pak::PakReader,
        smd::{Smd, SmdChunk, SmdHead, SmdTileMap},
    },
    errors::{AffError, SmdError},
};

fn read(bytes: &[u8]) -> Result<Smd, AffError> {
    Smd::from(&mut Cursor::new(bytes))
}

fn write(smd: &Smd) -> Vec<u8> {
    let mut out = Vec::new();
    smd.write(&mut out).unwrap();
    out
}

fn smd_error(result: Result<impl std::fmt::Debug, AffError>) -> SmdError {
    match result {
        Err(AffError::SmdError(e)) => e,
        other => panic!("expected an SmdError, got {other:?}"),
    }
}

/// Builds a form from raw chunks.
fn form(form_type: &[u8; 4], chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = form_type.to_vec();
    for (tag, data) in chunks {
        body.extend_from_slice(*tag);
        body.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
    }
    let mut out = b"FORM".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

fn le(values: &[[u8; 4]]) -> Vec<u8> {
    values.concat()
}

/// 3 x 2 tiles of 16 m from 100, 200, three of them shore tiles in a 2 x 2 atlas.
fn chunks() -> Vec<(&'static [u8; 4], Vec<u8>)> {
    let head = le(&[
        100f32.to_le_bytes(),
        200f32.to_le_bytes(),
        148f32.to_le_bytes(),
        232f32.to_le_bytes(),
        3u32.to_le_bytes(),
        10u32.to_le_bytes(),
        8u32.to_le_bytes(),
        16f32.to_le_bytes(),
        2f32.to_le_bytes(),
        2u32.to_le_bytes(),
        2u32.to_le_bytes(),
    ]);
    let mut tmap = le(&[3u32.to_le_bytes(), 2u32.to_le_bytes()]);
    for tile in [0u16, 1, 0, 2, 3, 0] {
        tmap.extend(tile.to_le_bytes());
    }
    vec![
        (b"VERS", 2u32.to_le_bytes().to_vec()),
        (b"HEAD", head),
        (b"TMAP", tmap),
    ]
}

fn expected() -> Smd {
    Smd {
        version: 2,
        head: SmdHead {
            min: [100.0, 200.0],
            max: [148.0, 232.0],
            tile_count: 3,
            slot_pixels: 10,
            tile_pixels: 8,
            tile_size: 16.0,
            meters_per_pixel: 2.0,
            atlas_slots: [2, 2],
        },
        tile_map: SmdTileMap {
            width: 3,
            height: 2,
            tiles: vec![0, 1, 0, 2, 3, 0],
        },
        unknown_chunks: vec![],
    }
}

fn slot(index: u16) -> Option<NonZeroU16> {
    NonZeroU16::new(index)
}

#[test]
fn smd_reads_and_writes_test() {
    let bytes = form(b"SMDF", &chunks());
    let smd = read(&bytes).unwrap();
    assert_eq!(smd, expected());
    assert_eq!(write(&smd), bytes);
}

#[test]
fn smd_lookup_test() {
    let smd = expected();
    assert_eq!(smd.tile_map.tile(1, 0), slot(1));
    assert_eq!(smd.tile_map.tile(0, 1), slot(2));
    assert_eq!(smd.tile_map.tile(0, 0), None);
    assert_eq!(smd.tile_map.tile(3, 0), None);
    assert_eq!(smd.tile_map.tile(0, 2), None);

    // Rows start at the lowest z.
    assert_eq!(smd.tile_at([116.0, 200.0]), slot(1));
    assert_eq!(smd.tile_at([131.9, 231.9]), slot(3));
    assert_eq!(smd.tile_at([110.0, 210.0]), None);
    for outside in [
        [99.9, 210.0],
        [148.0, 210.0],
        [120.0, 232.0],
        [f32::NAN, 210.0],
    ] {
        assert_eq!(smd.tile_at(outside), None, "{outside:?}");
    }

    // Slots row by row, including the border.
    let head = smd.head;
    assert_eq!(head.atlas_size(), [20, 20]);
    assert_eq!(head.atlas_slot(slot(1).unwrap()), [0, 0]);
    assert_eq!(head.atlas_slot(slot(2).unwrap()), [10, 0]);
    assert_eq!(head.atlas_slot(slot(3).unwrap()), [0, 10]);
}

#[test]
fn smd_unknown_chunks_test() {
    let mut chunks = chunks();
    chunks.insert(1, (b"NEWC", vec![1, 2, 3]));
    let smd = read(&form(b"SMDF", &chunks)).unwrap();
    assert_eq!(
        smd.unknown_chunks,
        [SmdChunk {
            tag: *b"NEWC",
            data: vec![1, 2, 3],
        }]
    );
    // Written after the known chunks.
    let new = chunks.remove(1);
    chunks.push(new);
    assert_eq!(write(&smd), form(b"SMDF", &chunks));
}

#[test]
fn smd_errors_test() {
    assert!(matches!(read(b"TOPO1234"), Err(AffError::InvalidFileError)));
    assert_eq!(
        smd_error(read(&form(b"TERR", &chunks()))),
        SmdError::UnknownFormType("TERR".into())
    );

    let mut missing = chunks();
    missing.remove(2);
    assert_eq!(
        smd_error(read(&form(b"SMDF", &missing))),
        SmdError::ChunkMissing("TMAP")
    );

    let mut version = chunks();
    version[0].1 = 3u32.to_le_bytes().to_vec();
    assert_eq!(
        smd_error(read(&form(b"SMDF", &version))),
        SmdError::UnsupportedVersion(3)
    );

    // Contents left over, cut short, or more tiles than the chunk holds.
    for (index, name) in [(1, "HEAD"), (2, "TMAP")] {
        let mut chunks = chunks();
        chunks[index].1.push(0);
        assert_eq!(
            smd_error(read(&form(b"SMDF", &chunks))),
            SmdError::InvalidChunk(name)
        );
        let len = chunks[index].1.len();
        chunks[index].1.truncate(len - 2);
        assert_eq!(
            smd_error(read(&form(b"SMDF", &chunks))),
            SmdError::InvalidChunk(name)
        );
    }
    let mut huge = chunks();
    huge[2].1[..8].copy_from_slice(&le(&[u32::MAX.to_le_bytes(), u32::MAX.to_le_bytes()]));
    assert_eq!(
        smd_error(read(&form(b"SMDF", &huge))),
        SmdError::InvalidChunk("TMAP")
    );

    let mut smd = expected();
    smd.tile_map.width = 4;
    assert_eq!(
        smd_error(smd.write(&mut Vec::new())),
        SmdError::Unwritable("TMAP")
    );
}

/// Workbench's shore map of the probe island, see `smd_probes/README.md`.
#[test]
fn smd_probe_test() {
    let bytes = std::fs::read("./tests/enfusion/smd_probes/probe.smd").unwrap();
    let smd = read(&bytes).unwrap();
    assert_eq!(write(&smd), bytes);

    // As logged: 5 x 5 tiles of 257.6 m from 4, -92, 23 of them in a 23 x 1 atlas.
    let head = smd.head;
    assert_eq!(head.min, [4.0, -92.0]);
    assert_eq!(head.max, [1292.0, 1196.0]);
    assert_eq!(head.tile_count, 23);
    assert_eq!([head.slot_pixels, head.tile_pixels], [132, 130]);
    assert!((head.tile_size - 257.6).abs() < 1e-3);
    assert!((head.meters_per_pixel - 1.981_54).abs() < 1e-5);
    assert_eq!(head.atlas_slots, [23, 1]);
    assert_eq!([smd.tile_map.width, smd.tile_map.height], [5, 5]);

    // Only the corner far from the island, at high x and z, has no shore.
    let mut no_shore = Vec::new();
    for z in 0..5 {
        for x in 0..5 {
            let center = [
                head.min[0] + (x as f32 + 0.5) * head.tile_size,
                head.min[1] + (z as f32 + 0.5) * head.tile_size,
            ];
            if smd.tile_at(center).is_none() {
                no_shore.push(center.map(f32::round));
            }
        }
    }
    assert_eq!(no_shore, [[1163.0, 810.0], [1163.0, 1067.0]]);
    // Numbered in tile map order.
    assert_eq!(smd.tile_at([400.0, 300.0]), slot(7));

    let atlas = RawEdds::from(&mut BufReader::new(
        File::open("./tests/enfusion/smd_probes/shoreDFNearAtlas.edds").unwrap(),
    ))
    .unwrap();
    let mipmap = atlas.mipmaps.last().unwrap();
    assert_eq!([mipmap.width, mipmap.height], [3036, 132]);
    assert_eq!(head.atlas_size(), [3036, 132]);
    assert_eq!(head.atlas_slot(slot(23).unwrap()), [22 * 132, 0]);
}

/// Reads every `.smd` in the paks below `AFF_SMD_SCAN_DIR`, e.g. the game's `addons` directory:
/// checks that it writes back identically and the relations the docs describe.
#[test]
fn smd_scan_dir_test() {
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

    let Ok(dir) = std::env::var("AFF_SMD_SCAN_DIR") else {
        return;
    };

    let mut paks = Vec::new();
    visit(Path::new(&dir), &mut paks);
    assert!(!paks.is_empty(), "no pak files in {dir}");

    let mut count = 0;
    for pak in paks {
        let reader = PakReader::from_stream(BufReader::new(File::open(&pak).unwrap())).unwrap();
        for entry in &reader.pak.entries {
            if !entry.path.ends_with(".smd") {
                continue;
            }
            let path = &entry.path;
            let bytes = reader.read_entry(entry).unwrap();
            let smd = read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert_eq!(write(&smd), bytes, "{path}");
            assert!(smd.unknown_chunks.is_empty(), "{path}");

            let head = smd.head;
            let map = &smd.tile_map;
            assert_eq!(head.slot_pixels, head.tile_pixels + 2, "{path}");
            let close = |a: f32, b: f32| (a - b).abs() < 0.01;
            assert!(
                close(
                    head.tile_size,
                    head.tile_pixels as f32 * head.meters_per_pixel
                ),
                "{path}"
            );
            for (axis, tiles) in [map.width, map.height].into_iter().enumerate() {
                let extent = head.max[axis] - head.min[axis];
                assert!(close(extent, tiles as f32 * head.tile_size), "{path}");
            }
            assert!(head.atlas_slots[0] * head.atlas_slots[1] >= head.tile_count);

            // Slots are numbered in tile map order.
            let used: Vec<_> = map.tiles.iter().copied().filter(|&t| t != 0).collect();
            assert_eq!(
                used,
                (1..=head.tile_count as u16).collect::<Vec<_>>(),
                "{path}"
            );

            println!(
                "{path}: {}x{} tiles of {} m, {} shore tiles, atlas {:?} px",
                map.width,
                map.height,
                head.tile_size,
                head.tile_count,
                head.atlas_size()
            );
            count += 1;
        }
    }
    assert!(count > 0, "no smd files in {dir}");
}
