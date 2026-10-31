//! Black box tests for the ttile reader and writer.
//!
//! `terr_probes/probe_0.ttile` and `probe_7.ttile` are the first and last tile of
//! `terr_probes/probe.terr`, see `terr_probes/README.md`.

use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use arma_file_formats::{
    enfusion::{
        pak::PakReader,
        terr::Terr,
        ttile::{
            MaterialBitmap, MaterialMap, MaterialNode, TileBlockLayers, TileBlockLod,
            TileBlockMaterials, TileGrass, TileGrassBlock, Ttile, bits_per_texel,
        },
    },
    errors::{AffError, TerrError},
};

const PROBE_DIR: &str = "./tests/enfusion/terr_probes";

fn read(bytes: &[u8]) -> Result<Ttile, AffError> {
    Ttile::from(&mut Cursor::new(bytes))
}

fn write(tile: &Ttile) -> Vec<u8> {
    let mut out = Vec::new();
    tile.write(&mut out).unwrap();
    out
}

fn terr_error(result: Result<impl std::fmt::Debug, AffError>) -> TerrError {
    match result {
        Err(AffError::TerrError(e)) => e,
        other => panic!("expected a TerrError, got {other:?}"),
    }
}

/// Builds a `TERR` form from raw chunks.
fn form(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = b"TERR".to_vec();
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

fn le16(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn le32(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn f32s(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A block of the probe: flat, materials not painted yet.
fn probe_block(x: u16, z: u16, index: u32) -> (TileBlockLayers, TileBlockMaterials) {
    (
        TileBlockLayers {
            block: index,
            materials: vec![0],
        },
        TileBlockMaterials {
            x,
            z,
            materials: vec![0],
            map: None,
        },
    )
}

#[test]
fn ttile_probe_test() {
    let terr = Terr::from(&mut BufReader::new(
        File::open(format!("{PROBE_DIR}/probe.terr")).unwrap(),
    ))
    .unwrap();

    // Tile 0 is the lowest x and z, tile 7 the highest of the 4 x 2 tiles.
    for (index, [tx, tz]) in [(0, [0, 0]), (7, [3, 1])] {
        let bytes = std::fs::read(format!("{PROBE_DIR}/probe_{index}.ttile")).unwrap();
        let tile = read(&bytes).unwrap();

        assert_eq!(tile.version, 9);
        // 64 grid cells per tile, all at 0 m, which is the highest value with "zero" height
        // at 100 %.
        assert_eq!(tile.vertices(), terr.head.tile_cells() + 1);
        assert_eq!(tile.vertices(), 65);
        assert!(tile.heights.iter().all(|&h| h == u16::MAX));
        assert!(terr.head.height(tile.height(64, 64).unwrap()).abs() < 1e-3);
        assert_eq!(tile.height(65, 0), None);

        // 2 x 2 blocks, errors not computed yet.
        assert_eq!(
            tile.lods,
            [TileBlockLod {
                min_height: 65535.0,
                max_height: 65535.0,
                unknown_8: 0.0,
                max_error: 0.0,
                lod_errors: [1e6; 4],
                center_x: 0.0,
                center_y: 65535.0 * 0.05,
                center_z: 0.0,
            }; 4]
        );
        // Grass clusters per block is 0 in the terr.
        assert_eq!(tile.grass, None);

        // Blocks are addressed in the whole terrain, 8 x 4 blocks.
        let (bx, bz) = (tx * 2, tz * 2);
        let blocks: [_; 4] = std::array::from_fn(|i| {
            let (x, z) = (bx + i as u16 % 2, bz + i as u16 / 2);
            probe_block(x, z, u32::from(z) * 8 + u32::from(x))
        });
        let (layers, materials): (Vec<_>, Vec<_>) = blocks.into_iter().unzip();
        assert_eq!(tile.layers, layers);
        assert_eq!(tile.materials, materials);
        assert!(tile.unknown_chunks.is_empty());

        assert_eq!(tile.materials[3].material_at(9, 63, 63, 64), Some(0));
        assert_eq!(tile.materials[3].material_at(9, 64, 0, 64), None);

        assert_eq!(write(&tile), bytes);
    }
}

/// A block with materials `3` and `7`: the second quadrant is a 4 x 4 bitmap.
fn compact_block() -> TileBlockMaterials {
    TileBlockMaterials {
        x: 1,
        z: 2,
        materials: vec![3, 7],
        map: Some(MaterialMap {
            unknown: 0,
            rect: [0; 4],
            root: MaterialNode::Node(0),
            nodes: vec![[
                MaterialNode::Material(0),
                MaterialNode::Bitmap(0),
                MaterialNode::Material(1),
                MaterialNode::Material(0),
            ]],
            bitmaps: vec![MaterialBitmap {
                rect: [0; 4],
                texels: 16,
                // Texel 0 and 15 are the second material, 1 bit each.
                data: vec![0b0000_0001, 0b1000_0000],
            }],
        }),
    }
}

#[test]
fn ttile_compact_materials_test() {
    let block = compact_block();
    let tile = Ttile {
        version: 9,
        heights: vec![1, 2, 3, 4],
        lods: Vec::new(),
        grass: None,
        layers: Vec::new(),
        materials: vec![block.clone()],
        unknown_chunks: Vec::new(),
    };
    let bytes = write(&tile);

    let bmat = [
        le16(&[1, 2, 2, 3, 7]),
        // One node, the root is node 0.
        le16(&[1, 0, 0x8000, 0x4000, 0x8001, 0x8000]),
        // One bitmap of 16 texels.
        le16(&[1]),
        le32(&[16]),
        vec![0b0000_0001, 0b1000_0000],
    ]
    .concat();
    let mut tmat = b"BMAT".to_vec();
    tmat.extend_from_slice(&(bmat.len() as u32).to_be_bytes());
    tmat.extend_from_slice(&bmat);
    let expected = form(&[
        (b"VERS", le32(&[9])),
        (b"HGHT", le16(&[1, 2, 3, 4])),
        (b"BERR", Vec::new()),
        (b"LRS2", Vec::new()),
        (b"TMAT", tmat),
    ]);
    assert_eq!(bytes, expected);
    assert_eq!(read(&bytes).unwrap(), tile);
    assert_eq!(tile.vertices(), 2);
    assert_eq!(tile.height(1, 0), Some(2));
    assert_eq!(tile.height(0, 1), Some(3));

    // Quadrants: (x0, z0), (x1, z0), (x0, z1), (x1, z1).
    let at = |x, z| block.material_at(9, x, z, 8);
    assert_eq!(at(0, 0), Some(3));
    assert_eq!(at(3, 3), Some(3));
    assert_eq!(at(4, 0), Some(7));
    assert_eq!(at(5, 0), Some(3));
    assert_eq!(at(7, 3), Some(7));
    assert_eq!(at(6, 3), Some(3));
    assert_eq!(at(0, 4), Some(7));
    assert_eq!(at(7, 7), Some(3));
    assert_eq!(at(8, 0), None);
    // Each bitmap texel covers 2 x 2 texels of a 16 texel block.
    assert_eq!(block.material_at(9, 9, 1, 16), Some(7));
    assert_eq!(block.material_at(9, 10, 1, 16), Some(3));

    assert_eq!(bits_per_texel(9, 2), 1);
    assert_eq!(bits_per_texel(9, 3), 2);
    assert_eq!(bits_per_texel(9, 4), 2);
    assert_eq!(bits_per_texel(9, 5), 3);
}

/// An old version 5 tile with every chunk.
fn version_5() -> Ttile {
    Ttile {
        version: 5,
        heights: vec![6553; 9],
        lods: vec![TileBlockLod {
            min_height: 6553.0,
            max_height: 6553.0,
            unknown_8: 1.0,
            max_error: 0.5,
            lod_errors: [0.1, 0.2, 0.4, 0.8],
            center_x: 16.0,
            center_y: 0.0,
            center_z: 48.0,
        }],
        grass: Some(TileGrass {
            block_clusters: 4,
            blocks: vec![TileGrassBlock {
                x: 0,
                z: 1,
                layers: 3,
                clusters: vec![0, 1, 2, 3],
            }],
        }),
        layers: vec![TileBlockLayers {
            block: 2,
            materials: vec![0, 1, 2],
        }],
        materials: vec![TileBlockMaterials {
            x: 0,
            z: 1,
            materials: vec![0, 1, 2],
            map: Some(MaterialMap {
                unknown: 0,
                rect: [0, 0, 7, 7],
                root: MaterialNode::Node(0),
                nodes: vec![[
                    MaterialNode::Bitmap(0),
                    MaterialNode::Material(2),
                    MaterialNode::Material(1),
                    MaterialNode::Material(0),
                ]],
                bitmaps: vec![MaterialBitmap {
                    rect: [0, 0, 3, 3],
                    texels: 16,
                    // 2 bits per texel: texel 0 is 1, texel 15 is 2.
                    data: 0x8000_0001u32.to_le_bytes().to_vec(),
                }],
            }),
        }],
        unknown_chunks: Vec::new(),
    }
}

#[test]
fn ttile_version_5_test() {
    let tile = version_5();
    let bytes = write(&tile);

    let qtre = le32(&[
        // One node, root, unknown, rectangle.
        1,
        0,
        0,
        0,
        0,
        7,
        7,
        0x4000_0000,
        0x8000_0002,
        0x8000_0001,
        0x8000_0000,
        // One bitmap: rectangle, materials, texels, u32s.
        1,
        0,
        0,
        3,
        3,
        3,
        16,
        1,
        0x8000_0001,
    ]);
    let mut bmat = le16(&[0, 1, 3, 0, 1, 2]);
    bmat.extend_from_slice(b"QTRE");
    bmat.extend_from_slice(&(qtre.len() as u32).to_be_bytes());
    bmat.extend_from_slice(&qtre);
    let mut tmat = b"BMAT".to_vec();
    tmat.extend_from_slice(&(bmat.len() as u32).to_be_bytes());
    tmat.extend_from_slice(&bmat);

    let expected = form(&[
        (b"VERS", le32(&[5])),
        (b"HGHT", le16(&[6553; 9])),
        (
            b"BERR",
            f32s(&[
                6553.0, 6553.0, 1.0, 0.5, 0.1, 0.2, 0.4, 0.8, 16.0, 0.0, 48.0,
            ]),
        ),
        (b"GCTD", [le16(&[5, 0, 1]), vec![3, 0, 1, 2, 3]].concat()),
        // `u16` block index before version 7.
        (b"LRS2", le16(&[2, 3, 0, 1, 2])),
        (b"TMAT", tmat),
    ]);
    assert_eq!(bytes, expected);
    assert_eq!(read(&bytes).unwrap(), tile);

    let block = &tile.materials[0];
    assert_eq!(bits_per_texel(5, 3), 2);
    assert_eq!(block.material_at(5, 0, 0, 8), Some(1));
    assert_eq!(block.material_at(5, 1, 0, 8), Some(0));
    assert_eq!(block.material_at(5, 3, 3, 8), Some(2));
    assert_eq!(block.material_at(5, 4, 0, 8), Some(2));
    assert_eq!(block.material_at(5, 0, 4, 8), Some(1));
    assert_eq!(block.material_at(5, 4, 4, 8), Some(0));

    // Version 7 stores `u32` block indices.
    let mut tile = tile;
    tile.version = 7;
    tile.layers[0].block = 0x1_0000;
    let tile_7 = read(&write(&tile)).unwrap();
    assert_eq!(tile_7, tile);
}

#[test]
fn ttile_errors_test() {
    let tile = |chunks: &[(&[u8; 4], Vec<u8>)]| terr_error(read(&form(chunks)));
    let vers = |v: u32| (b"VERS", le32(&[v]));
    let empty = |tag| (tag, Vec::new());

    assert_eq!(
        tile(&[vers(9), empty(b"HGHT"), empty(b"BERR"), empty(b"LRS2")]),
        TerrError::ChunkMissing("TMAT")
    );
    // 3 heights are no square.
    assert_eq!(
        tile(&[
            vers(9),
            (b"HGHT", le16(&[1, 2, 3])),
            empty(b"BERR"),
            empty(b"LRS2"),
            empty(b"TMAT")
        ]),
        TerrError::InvalidChunk("HGHT")
    );
    // A version 9 block is 36 bytes.
    assert_eq!(
        tile(&[
            vers(9),
            empty(b"HGHT"),
            (b"BERR", vec![0; 44]),
            empty(b"LRS2"),
            empty(b"TMAT")
        ]),
        TerrError::InvalidChunk("BERR")
    );
    assert_eq!(
        tile(&[
            vers(9),
            empty(b"HGHT"),
            empty(b"BERR"),
            empty(b"LRS2"),
            (b"TMAT", [b"QTRE".to_vec(), vec![0; 4]].concat())
        ]),
        TerrError::InvalidChunk("TMAT")
    );
    // 16 texels of 1 bit need 2 bytes.
    let bmat = [
        le16(&[1, 2, 2, 3, 7, 1, 0, 0x8000, 0x4000, 0x8001, 0x8000, 1]),
        le32(&[16]),
        vec![1],
    ]
    .concat();
    let tmat = [
        b"BMAT".to_vec(),
        (bmat.len() as u32).to_be_bytes().to_vec(),
        bmat,
    ]
    .concat();
    assert_eq!(
        tile(&[
            vers(9),
            empty(b"HGHT"),
            empty(b"BERR"),
            empty(b"LRS2"),
            (b"TMAT", tmat)
        ]),
        TerrError::InvalidChunk("TMAT")
    );
    let mut tile_9 = Ttile {
        version: 9,
        heights: Vec::new(),
        lods: Vec::new(),
        grass: None,
        layers: Vec::new(),
        materials: vec![compact_block()],
        unknown_chunks: Vec::new(),
    };

    // Writing data the version can't hold.
    let unwritable = |tile: &Ttile| terr_error(tile.write(&mut Vec::new()));
    let map = tile_9.materials[0].map.as_mut().unwrap();
    map.bitmaps[0].data.pop();
    assert_eq!(unwritable(&tile_9), TerrError::Unwritable("TMAT"));
    let map = tile_9.materials[0].map.as_mut().unwrap();
    map.bitmaps[0].data.push(0);
    map.root = MaterialNode::Node(0x4000);
    assert_eq!(unwritable(&tile_9), TerrError::Unwritable("TMAT"));

    let mut tile_5 = version_5();
    tile_5.grass.as_mut().unwrap().blocks[0].clusters.pop();
    assert_eq!(unwritable(&tile_5), TerrError::Unwritable("GCTD"));
    let mut tile_5 = version_5();
    tile_5.materials[0].map.as_mut().unwrap().bitmaps[0]
        .data
        .push(0);
    assert_eq!(unwritable(&tile_5), TerrError::Unwritable("TMAT"));
    let mut tile_5 = version_5();
    tile_5.layers[0].block = 0x1_0000;
    assert!(tile_5.write(&mut Vec::new()).is_err());
}

/// Reads every `.ttile` in the paks below `AFF_TERR_SCAN_DIR`, e.g. the game's `addons`
/// directory: checks that it writes back identically and fits the `.terr` next to it.
#[test]
fn ttile_scan_dir_test() {
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
            let terr = Terr::from(&mut Cursor::new(reader.read_entry(entry).unwrap())).unwrap();
            let (dir, name) = stem.rsplit_once('/').unwrap();
            let prefix = format!("{dir}/.Data/{name}_");
            let [tiles_x, _] = terr.head.tiles();
            let blocks = terr.head.blocks_per_tile;

            for tile_entry in reader
                .pak
                .entries
                .iter()
                .filter(|e| e.path.starts_with(&prefix) && e.path.ends_with(".ttile"))
            {
                let path = &tile_entry.path;
                let index: u32 = path[prefix.len()..path.len() - ".ttile".len()]
                    .parse()
                    .unwrap();
                let bytes = reader.read_entry(tile_entry).unwrap();
                let tile = read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
                assert_eq!(write(&tile), bytes, "{path}");

                assert_eq!(tile.version, terr.version, "{path}");
                assert_eq!(tile.vertices(), terr.head.tile_cells() + 1, "{path}");
                assert_eq!(tile.lods.len() as u32, blocks * blocks, "{path}");
                assert_eq!(tile.layers.len() as u32, blocks * blocks, "{path}");
                assert_eq!(tile.materials.len() as u32, blocks * blocks, "{path}");
                for (i, (layers, materials)) in tile.layers.iter().zip(&tile.materials).enumerate()
                {
                    let x = index % tiles_x * blocks + i as u32 % blocks;
                    let z = index / tiles_x * blocks + i as u32 / blocks;
                    assert_eq!(layers.block, z * tiles_x * blocks + x, "{path}");
                    assert_eq!(
                        (u32::from(materials.x), u32::from(materials.z)),
                        (x, z),
                        "{path}"
                    );
                    assert!(
                        materials
                            .materials
                            .iter()
                            .all(|m| layers.materials.contains(m)),
                        "{path}"
                    );
                    for z in 0..128 {
                        for x in 0..128 {
                            let material = materials.material_at(tile.version, x, z, 128);
                            assert!(
                                material.is_some_and(|m| usize::from(m) < terr.materials.len()),
                                "{path}"
                            );
                        }
                    }
                }
                for lod in tile.lods.iter().filter(|l| l.lod_errors != [1e6; 4]) {
                    let whole = |v: f32| (v - v.round()).abs() < 0.02;
                    let max = lod.max_error / terr.head.height_scale;
                    let errors: Vec<f32> = lod.lod_errors.iter().map(|e| e * max).collect();
                    assert!(errors.iter().chain([&max]).all(|&e| whole(e)), "{path}");
                    assert!(
                        errors
                            .iter()
                            .chain([&max])
                            .is_sorted_by(|a, b| a.round() <= b.round()),
                        "{path}"
                    );
                    assert!(whole(lod.unknown_8), "{path}");
                    assert!(lod.unknown_8 <= lod.max_height - lod.min_height, "{path}");
                }
                if let Some(grass) = &tile.grass {
                    let clusters = terr.grass.unwrap().clusters_per_block;
                    assert_eq!(grass.block_clusters, clusters * clusters, "{path}");
                    for block in &grass.blocks {
                        let layers = block.clusters.iter().fold(0, |a, c| a | c);
                        assert_eq!(block.layers, layers, "{path}");
                    }
                }
                count += 1;
            }
        }
    }
    println!("{count} tiles");
    assert!(count > 0, "no ttile files in {dir}");
}
