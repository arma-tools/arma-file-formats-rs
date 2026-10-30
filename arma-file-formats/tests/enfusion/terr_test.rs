//! Black box tests for the terr reader and writer.
//!
//! `terr_probes/probe.terr` was created by Arma Reforger Workbench with the values listed in
//! `terr_probes/README.md`, each different from the defaults so every field can be told apart.

use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use arma_file_formats::{
    enfusion::{
        pak::PakReader,
        terr::{Terr, TerrChunk, TerrGrass, TerrHead, TerrTextureSizes},
    },
    errors::{AffError, TerrError},
};

const PROBE_PATH: &str = "./tests/enfusion/terr_probes/probe.terr";
const DEFAULT_MATERIAL: &str = "{E755137CEA663544}system/terrain/default.emat";

fn read(bytes: &[u8]) -> Result<Terr, AffError> {
    Terr::from(&mut Cursor::new(bytes))
}

fn write(terr: &Terr) -> Vec<u8> {
    let mut out = Vec::new();
    terr.write(&mut out).unwrap();
    out
}

fn terr_error(bytes: &[u8]) -> TerrError {
    match read(bytes) {
        Err(AffError::TerrError(e)) => e,
        other => panic!("expected a TerrError, got {other:?}"),
    }
}

/// Builds a terr file from raw chunks.
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

fn material(path: &str) -> Vec<u8> {
    let mut out = ((path.len() + 1) as u32).to_le_bytes().to_vec();
    out.extend_from_slice(path.as_bytes());
    out.push(0);
    out
}

/// The game's `Editor/Slots/Slots/Flat` terrain, an old version 5 terrain.
fn version_5() -> Terr {
    Terr {
        version: 5,
        head: TerrHead {
            vertices_x: 129,
            vertices_z: 129,
            block_vertices: 33,
            blocks_per_tile: 4,
            grid_cell_size: 1.0,
            height_scale: 0.031_25,
            height_offset: -204.781_25,
            unknown_28: 5,
        },
        texture_sizes: TerrTextureSizes {
            super_texture: 128,
            normal: 0,
            layer: 128,
        },
        surface_bits: 8,
        materials: vec![
            DEFAULT_MATERIAL.to_owned(),
            "{5E80F5F438B08C00}Terrains/Common/Surfaces/Grass_01.emat".to_owned(),
        ],
        grass: Some(TerrGrass {
            cluster_size: 10.0,
            clusters_per_block: 3,
        }),
        unknown_chunks: Vec::new(),
    }
}

#[test]
fn terr_probe_test() {
    let bytes = std::fs::read(PROBE_PATH).unwrap();
    let terr = read(&bytes).unwrap();

    assert_eq!(terr.version, 9);
    assert_eq!(
        terr.head,
        TerrHead {
            // Width 256 and height 128 grid cells.
            vertices_x: 257,
            vertices_z: 129,
            block_vertices: 33,
            blocks_per_tile: 2,
            grid_cell_size: 1.5,
            height_scale: 0.05,
            // Zero height at 100 %: the whole 16 bit range lies below 0 m.
            height_offset: -65535.0 * 0.05,
            unknown_28: 9,
        }
    );
    // A new terrain has no textures and no grass clusters yet.
    assert_eq!(
        terr.texture_sizes,
        TerrTextureSizes {
            super_texture: 0,
            normal: 0,
            layer: 0,
        }
    );
    // 5 bits (7 surfaces).
    assert_eq!(terr.surface_bits, 5);
    assert_eq!(terr.materials, [DEFAULT_MATERIAL]);
    assert_eq!(
        terr.grass,
        Some(TerrGrass {
            cluster_size: 2.5,
            clusters_per_block: 0,
        })
    );
    assert!(terr.unknown_chunks.is_empty());

    // Workbench wrote `probe_0.ttile` to `probe_7.ttile`.
    assert_eq!(terr.head.tile_cells(), 64);
    assert_eq!(terr.head.tiles(), [4, 2]);
    assert_eq!(terr.head.tile_count(), 8);
    assert_eq!(terr.head.size_meters(), [384.0, 192.0]);
    assert_eq!(terr.head.height(0), -3276.75);
    assert!(terr.head.height(u16::MAX).abs() < 1e-3);

    assert_eq!(write(&terr), bytes);
}

#[test]
fn terr_version_5_test() {
    let terr = version_5();
    let bytes = write(&terr);

    // Before version 9 `MATS` starts with the first material, without surface bits.
    let expected = form(
        b"TERR",
        &[
            (b"VERS", 5u32.to_le_bytes().to_vec()),
            (
                b"HEAD",
                [
                    129u32.to_le_bytes(),
                    129u32.to_le_bytes(),
                    33u32.to_le_bytes(),
                    4u32.to_le_bytes(),
                    1.0f32.to_le_bytes(),
                    0.031_25f32.to_le_bytes(),
                    (-204.781_25f32).to_le_bytes(),
                    5u32.to_le_bytes(),
                ]
                .concat(),
            ),
            (b"TEXS", vec![0x80, 0, 0, 0, 0x80, 0]),
            (
                b"MATS",
                [
                    material(DEFAULT_MATERIAL),
                    material("{5E80F5F438B08C00}Terrains/Common/Surfaces/Grass_01.emat"),
                ]
                .concat(),
            ),
            (b"GRAS", vec![0, 0, 0x20, 0x41, 3, 0]),
        ],
    );
    assert_eq!(bytes, expected);
    assert_eq!(read(&bytes).unwrap(), terr);

    // Default height range: 10 % below 0 m.
    assert_eq!(terr.head.height(6553), 0.0);
    assert_eq!(terr.head.tile_count(), 1);
}

#[test]
fn terr_unknown_chunks_test() {
    let mut terr = version_5();
    terr.grass = None;
    terr.unknown_chunks = vec![
        TerrChunk {
            tag: *b"ABCD",
            data: vec![1, 2, 3],
        },
        TerrChunk {
            tag: *b"EMPT",
            data: Vec::new(),
        },
    ];

    let bytes = write(&terr);
    assert_eq!(read(&bytes).unwrap(), terr);
}

#[test]
fn terr_errors_test() {
    let vers = |v: u32| (b"VERS", v.to_le_bytes().to_vec());
    let head = (b"HEAD", vec![0; 32]);
    let texs = (b"TEXS", vec![0; 6]);
    let mats = (b"MATS", material(DEFAULT_MATERIAL));

    assert!(matches!(
        read(b"RIFF\0\0\0\x04TERR"),
        Err(AffError::InvalidFileError)
    ));
    assert_eq!(
        terr_error(&form(b"XOB9", &[])),
        TerrError::UnknownFormType("XOB9".to_owned())
    );
    assert_eq!(
        terr_error(&form(b"TERR", &[vers(5), texs.clone(), mats.clone()])),
        TerrError::ChunkMissing("HEAD")
    );
    assert_eq!(
        terr_error(&form(
            b"TERR",
            &[vers(5), (b"HEAD", vec![0; 28]), texs.clone(), mats.clone()]
        )),
        TerrError::InvalidChunk("HEAD")
    );
    assert_eq!(
        terr_error(&form(
            b"TERR",
            &[vers(5), (b"HEAD", vec![0; 36]), texs.clone(), mats.clone()]
        )),
        TerrError::InvalidChunk("HEAD")
    );

    // Version 9 expects the surface bits first, so the material length is off by two bytes.
    assert_eq!(
        terr_error(&form(
            b"TERR",
            &[vers(9), head.clone(), texs.clone(), mats.clone()]
        )),
        TerrError::InvalidChunk("MATS")
    );

    let mut unterminated = material(DEFAULT_MATERIAL);
    unterminated.pop();
    unterminated[0] -= 1;
    assert_eq!(
        terr_error(&form(
            b"TERR",
            &[vers(5), head.clone(), texs.clone(), (b"MATS", unterminated)]
        )),
        TerrError::InvalidChunk("MATS")
    );

    let mut truncated = material(DEFAULT_MATERIAL);
    truncated.extend_from_slice(&[0xFF, 0, 0, 0, b'a']);
    assert_eq!(
        terr_error(&form(b"TERR", &[vers(5), head, texs, (b"MATS", truncated)])),
        TerrError::InvalidChunk("MATS")
    );
}

/// Reads every `.terr` in the paks below `AFF_TERR_SCAN_DIR`, e.g. the game's `addons` directory:
/// checks that it writes back identically and that its tile count matches the `.ttile` files
/// next to it.
#[test]
fn terr_scan_dir_test() {
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

    let Ok(dir) = std::env::var("AFF_TERR_SCAN_DIR") else {
        return;
    };

    let mut paks = Vec::new();
    visit(Path::new(&dir), &mut paks);
    assert!(!paks.is_empty(), "no pak files in {dir}");

    let mut count = 0;
    for pak in paks {
        let reader = PakReader::from_stream(BufReader::new(File::open(&pak).unwrap())).unwrap();
        for entry in &reader.pak.entries {
            let Some(stem) = entry.path.strip_suffix(".terr") else {
                continue;
            };
            let bytes = reader.read_entry(entry).unwrap();
            let terr = read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", entry.path));
            assert_eq!(write(&terr), bytes, "{}", entry.path);

            let (dir, name) = stem.rsplit_once('/').unwrap();
            let tile_prefix = format!("{dir}/.Data/{name}_");
            let tiles = reader
                .pak
                .entries
                .iter()
                .filter(|e| e.path.starts_with(&tile_prefix) && e.path.ends_with(".ttile"))
                .count();
            assert_eq!(tiles, terr.head.tile_count() as usize, "{}", entry.path);

            println!(
                "{}: version {}, {:?} tiles, {} materials",
                entry.path,
                terr.version,
                terr.head.tiles(),
                terr.materials.len()
            );
            count += 1;
        }
    }
    assert!(count > 0, "no terr files in {dir}");
}
