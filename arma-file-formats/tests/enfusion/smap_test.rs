//! Black box tests for the smap reader and writer.
//!
//! Game files can't be committed, the inputs are assembled by hand from the layout of the game's
//! smaps. `smap_scan_dir_test` reads the game's files.

use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use arma_file_formats::{
    enfusion::{
        pak::PakReader,
        smap::{
            FreshwaterSound, GridCell, GridLayer, Smap, SmapChunk, SoundBlock, SoundLayer,
            SoundMapGrid, SoundMapLayers, TerrainSound,
        },
    },
    errors::{AffError, SmapError},
};

fn read(bytes: &[u8]) -> Result<Smap, AffError> {
    Smap::from(&mut Cursor::new(bytes))
}

fn write(smap: &Smap) -> Vec<u8> {
    let mut out = Vec::new();
    smap.write(&mut out).unwrap();
    out
}

fn smap_error(result: Result<impl std::fmt::Debug, AffError>) -> SmapError {
    match result {
        Err(AffError::SmapError(e)) => e,
        other => panic!("expected an SmapError, got {other:?}"),
    }
}

fn u32s(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn f32s(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn u16s(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// 3 x 2 cells of 10 m from 100, 200. Cell 4 (x 1, z 1) is near fresh water.
fn grid_bytes() -> Vec<u8> {
    let mut out = u32s(&[6, 2, 3]);
    out.extend(f32s(&[10.0, 100.0, -50.0, 200.0, 130.0, 0.0, 220.0]));
    for cell in 0..6 {
        let freshwater: i32 = if cell == 4 { 0 } else { -1 };
        out.extend(u32s(&[cell]));
        out.extend(freshwater.to_le_bytes());
    }
    // Terrain: sea, land, forest.
    out.extend(u32s(&[3, 6, 0]));
    out.extend([255, 0, 0, 40, 0, 0, 0, 0, 0, 0, 12, 0, 0, 200, 0, 0, 15, 1]);
    out.extend(u16s(&[0, 0, 0, 1, 2, 1]));
    // Freshwater: 25 sub-cells, a river along the lowest row.
    out.extend(u32s(&[2, 25, 1]));
    out.extend([0, 0, 0, 0, 90, 30, 0, 7]);
    let mut subcells = vec![1; 5];
    subcells.extend([0; 20]);
    out.extend(u16s(&subcells));
    out
}

fn expected_grid() -> SoundMapGrid {
    let mut subcells = vec![1; 5];
    subcells.extend([0; 20]);
    SoundMapGrid {
        cells_z: 2,
        cells_x: 3,
        cell_size: 10.0,
        min: [100.0, -50.0, 200.0],
        max: [130.0, 0.0, 220.0],
        cells: (0..6)
            .map(|terrain| GridCell {
                terrain,
                freshwater: if terrain == 4 { 0 } else { -1 },
            })
            .collect(),
        terrain: GridLayer {
            records: vec![
                TerrainSound {
                    sea: 255,
                    coast: 40,
                    ..TerrainSound::default()
                },
                TerrainSound {
                    height: 12,
                    ..TerrainSound::default()
                },
                TerrainSound {
                    forest: 200,
                    height: 15,
                    unknown_5: 1,
                    ..TerrainSound::default()
                },
            ],
            indices: vec![0, 0, 0, 1, 2, 1],
        },
        freshwater: GridLayer {
            records: vec![
                FreshwaterSound::default(),
                FreshwaterSound {
                    river: 90,
                    river_slope: 30,
                    lake: 0,
                    unknown_3: 7,
                },
            ],
            indices: subcells,
        },
    }
}

#[test]
fn smap_grid_test() {
    let bytes = grid_bytes();
    let smap = read(&bytes).unwrap();
    assert_eq!(smap, Smap::Grid(expected_grid()));
    assert_eq!(write(&smap), bytes);
}

#[test]
fn smap_grid_lookup_test() {
    let grid = expected_grid();
    assert_eq!(grid.cell_index([100.0, 200.0]), Some(0));
    assert_eq!(grid.cell_index([129.9, 219.9]), Some(5));
    for outside in [
        [99.9, 205.0],
        [130.0, 205.0],
        [105.0, 220.0],
        [f32::NAN, 205.0],
    ] {
        assert_eq!(grid.cell_index(outside), None, "{outside:?}");
    }
    assert_eq!(grid.terrain_at([115.0, 215.0]).unwrap().forest, 200);
    assert_eq!(grid.terrain_at([105.0, 205.0]).unwrap().sea, 255);

    // Sub-cells of 2 m, row by row along x from the lowest z.
    let river = Some(FreshwaterSound {
        river: 90,
        river_slope: 30,
        lake: 0,
        unknown_3: 7,
    });
    assert_eq!(grid.freshwater_at([110.5, 210.5]), river);
    assert_eq!(grid.freshwater_at([119.9, 211.9]), river);
    assert_eq!(
        grid.freshwater_at([110.5, 212.5]),
        Some(FreshwaterSound::default())
    );
    assert_eq!(grid.freshwater_at([105.0, 205.0]), None);
}

#[test]
fn smap_grid_errors_test() {
    let bytes = grid_bytes();
    for len in [0, 10, 100, bytes.len() - 1] {
        assert_eq!(
            smap_error(read(&bytes[..len])),
            SmapError::InvalidGrid,
            "{len}"
        );
    }
    // The cell count doesn't match the grid.
    let mut count = bytes.clone();
    count[4] = 3;
    assert_eq!(smap_error(read(&count)), SmapError::InvalidGrid);
    // Layers out of order.
    let mut id = bytes;
    let terrain = 40 + 6 * 8;
    id[terrain + 8] = 1;
    assert_eq!(smap_error(read(&id)), SmapError::InvalidGrid);

    let mut grid = expected_grid();
    grid.cells.pop();
    assert_eq!(
        smap_error(Smap::Grid(grid).write(&mut Vec::new())),
        SmapError::Unwritable("cells")
    );
}

fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(data);
    out
}

fn form(chunks: &[Vec<u8>]) -> Vec<u8> {
    let body = [b"SMAP".to_vec(), chunks.concat()].concat();
    [
        b"FORM".to_vec(),
        (body.len() as u32).to_be_bytes().to_vec(),
        body,
    ]
    .concat()
}

/// A block of 2 x 2 tiles of 1 m, with `data` an LZ4 block of `size` bytes.
fn block(x: u16, z: u16, size: u32, data: &[u8]) -> Vec<u8> {
    let (x0, z0) = (f32::from(x) * 2.0, f32::from(z) * 2.0);
    let mut out = u16s(&[x, z]);
    out.extend(f32s(&[x0, -10.0, z0, x0 + 2.0, 50.0, z0 + 2.0]));
    out.extend(u16s(&[2, 1]));
    out.extend(u32s(&[size, size, data.len() as u32]));
    out.extend(data);
    chunk(b"BLCK", &out)
}

/// Two blocks of 2 m: one uniform 0, one with 4 literals.
fn layers_chunks() -> Vec<Vec<u8>> {
    let mut head = chunk(b"VER\0", &1u16.to_le_bytes());
    head.extend(f32s(&[0.0, -10.0, 0.0, 4.0, 50.0, 2.0]));
    head.extend(u32s(&[2, 1]));
    head.extend(u16s(&[2, 1]));
    let blocks = [
        block(1, 0, 4, &[0x40, 1, 2, 3, 4]),
        block(0, 0, 1, &[0x10, 0]),
    ]
    .concat();
    let layer = [chunk(b"HEAD", &head), chunk(b"BLKS", &blocks)].concat();
    let mut top_head = f32s(&[0.0, -10.0, 0.0, 4.0, 50.0, 2.0]);
    top_head.extend(u32s(&[1]));
    vec![
        chunk(b"VERS", &u32s(&[1])),
        chunk(b"HEAD", &top_head),
        chunk(b"L$10", &layer),
    ]
}

fn expected_layers() -> SoundMapLayers {
    let block = |x: u16, size: u32, data: Vec<u8>| SoundBlock {
        x,
        z: 0,
        min: [f32::from(x) * 2.0, -10.0, 0.0],
        max: [f32::from(x) * 2.0 + 2.0, 50.0, 2.0],
        resolution: 2,
        tile_size: 1,
        size,
        unknown_36: size,
        data,
    };
    SoundMapLayers {
        version: 1,
        min: [0.0, -10.0, 0.0],
        max: [4.0, 50.0, 2.0],
        unknown_24: 1,
        layers: vec![SoundLayer {
            tag: *b"L$10",
            version: 1,
            min: [0.0, -10.0, 0.0],
            max: [4.0, 50.0, 2.0],
            block_size: 2,
            tile_size: 1,
            blocks_x: 2,
            blocks_z: 1,
            blocks: vec![
                block(1, 4, vec![0x40, 1, 2, 3, 4]),
                block(0, 1, vec![0x10, 0]),
            ],
        }],
        unknown_chunks: vec![],
    }
}

#[test]
fn smap_layers_test() {
    let bytes = form(&layers_chunks());
    let smap = read(&bytes).unwrap();
    assert_eq!(smap, Smap::Layers(expected_layers()));
    assert_eq!(write(&smap), bytes);
}

#[test]
fn smap_layers_values_test() {
    let layers = expected_layers();
    let layer = &layers.layers[0];
    assert_eq!(layer.blocks[0].values().unwrap(), [1, 2, 3, 4]);
    // A uniform block is expanded.
    assert_eq!(layer.blocks[1].values().unwrap(), [0; 4]);

    // Tiles row by row along x from the lowest z of the block.
    assert_eq!(layer.value_at([2.5, 0.5]).unwrap(), Some(1));
    assert_eq!(layer.value_at([3.5, 0.5]).unwrap(), Some(2));
    assert_eq!(layer.value_at([2.5, 1.5]).unwrap(), Some(3));
    assert_eq!(layer.value_at([0.5, 1.5]).unwrap(), Some(0));
    assert_eq!(layer.value_at([4.5, 0.5]).unwrap(), None);
    assert_eq!(layer.value_at([0.5, -0.5]).unwrap(), None);

    let mut block = layer.blocks[0].clone();
    block.set_values(&[9, 8, 7, 6]).unwrap();
    assert_eq!(block.values().unwrap(), [9, 8, 7, 6]);
    block.set_values(&[5; 4]).unwrap();
    assert_eq!((block.size, block.unknown_36), (1, 1));
    assert_eq!(block.values().unwrap(), [5; 4]);
    assert_eq!(
        smap_error(block.set_values(&[1; 3])),
        SmapError::Unwritable("block values")
    );

    let mut broken = layer.blocks[0].clone();
    broken.data = vec![0xf0];
    assert_eq!(smap_error(broken.values()), SmapError::Decompression(1, 0));
    broken.data = vec![0x30, 1, 2, 3];
    broken.size = 3;
    assert_eq!(smap_error(broken.values()), SmapError::Decompression(1, 0));
}

#[test]
fn smap_layers_unknown_chunks_test() {
    let mut chunks = layers_chunks();
    chunks.insert(2, chunk(b"NEWC", &[1, 2, 3]));
    let Smap::Layers(layers) = read(&form(&chunks)).unwrap() else {
        panic!("not layers");
    };
    assert_eq!(
        layers.unknown_chunks,
        [SmapChunk {
            tag: *b"NEWC",
            data: vec![1, 2, 3],
        }]
    );
    // Written after the known chunks.
    let new = chunks.remove(2);
    chunks.push(new);
    assert_eq!(write(&Smap::Layers(layers)), form(&chunks));
}

#[test]
fn smap_layers_errors_test() {
    let wrong_type = [
        b"FORM".to_vec(),
        4u32.to_be_bytes().to_vec(),
        b"TERR".to_vec(),
    ]
    .concat();
    assert_eq!(
        smap_error(read(&wrong_type)),
        SmapError::UnknownFormType("TERR".into())
    );

    let mut chunks = layers_chunks();
    chunks.remove(1);
    assert_eq!(
        smap_error(read(&form(&chunks))),
        SmapError::ChunkMissing("HEAD")
    );

    let mut chunks = layers_chunks();
    chunks[0] = chunk(b"VERS", &u32s(&[2]));
    assert_eq!(
        smap_error(read(&form(&chunks))),
        SmapError::UnsupportedVersion(2)
    );

    let mut chunks = layers_chunks();
    chunks[1].push(0);
    let len = chunks[1].len() as u32 - 8;
    chunks[1][4..8].copy_from_slice(&len.to_be_bytes());
    assert_eq!(
        smap_error(read(&form(&chunks))),
        SmapError::InvalidChunk("HEAD")
    );

    // A layer without BLKS.
    let mut chunks = layers_chunks();
    let head_size = u32::from_be_bytes(chunks[2][12..16].try_into().unwrap()) as usize;
    let layer = chunks[2][8..8 + 8 + head_size].to_vec();
    chunks[2] = chunk(b"L$10", &layer);
    assert_eq!(
        smap_error(read(&form(&chunks))),
        SmapError::InvalidChunk("layer")
    );
}

/// Reads every `.smap` in the paks below `AFF_SMAP_SCAN_DIR`, e.g. the game's `addons`
/// directory: checks that it writes back identically and the structure the docs describe.
#[test]
fn smap_scan_dir_test() {
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

    let Ok(dir) = std::env::var("AFF_SMAP_SCAN_DIR") else {
        return;
    };

    let mut paks = Vec::new();
    visit(Path::new(&dir), &mut paks);
    assert!(!paks.is_empty(), "no pak files in {dir}");

    let mut count = 0;
    for pak in paks {
        let reader = PakReader::from_stream(BufReader::new(File::open(&pak).unwrap())).unwrap();
        for entry in &reader.pak.entries {
            if !entry.path.ends_with(".smap") {
                continue;
            }
            let path = &entry.path;
            let bytes = reader.read_entry(entry).unwrap();
            let smap = read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert_eq!(write(&smap), bytes, "{path}");

            match &smap {
                Smap::Grid(grid) => {
                    let cells = grid.cells.len();
                    // Terrain per cell, freshwater numbered in cell order.
                    let mut next = 0;
                    for (i, cell) in grid.cells.iter().enumerate() {
                        assert_eq!(cell.terrain as usize, i, "{path}");
                        if cell.freshwater >= 0 {
                            assert_eq!(cell.freshwater, next, "{path}");
                            next += 1;
                        }
                    }
                    assert_eq!(grid.terrain.indices.len(), cells, "{path}");
                    assert_eq!(grid.freshwater.indices.len(), next as usize * 25, "{path}");
                    let terrain = grid.terrain.records.len();
                    let freshwater = grid.freshwater.records.len();
                    assert!(
                        grid.terrain
                            .indices
                            .iter()
                            .all(|&i| usize::from(i) < terrain)
                    );
                    assert!(
                        grid.freshwater
                            .indices
                            .iter()
                            .all(|&i| usize::from(i) < freshwater)
                    );
                    for axis in [0, 2] {
                        let count = if axis == 0 {
                            grid.cells_x
                        } else {
                            grid.cells_z
                        };
                        let extent = grid.max[axis] - grid.min[axis];
                        assert!(
                            (extent - count as f32 * grid.cell_size).abs() < 0.1,
                            "{path}"
                        );
                    }
                    println!(
                        "{path}: {} x {} cells, {} terrain and {} freshwater records",
                        grid.cells_x,
                        grid.cells_z,
                        grid.terrain.records.len(),
                        grid.freshwater.records.len()
                    );
                }
                Smap::Layers(layers) => {
                    for layer in &layers.layers {
                        let blocks = usize::from(layer.blocks_x) * usize::from(layer.blocks_z);
                        assert_eq!(layer.blocks.len(), blocks, "{path}");
                        for block in &layer.blocks {
                            let values = block.values().unwrap_or_else(|e| panic!("{path}: {e}"));
                            assert_eq!(values.len(), 128 * 128, "{path}");
                            let size = layer.block_size as f32;
                            assert_eq!(block.min[0], layer.min[0] + f32::from(block.x) * size);
                            assert_eq!(block.min[2], layer.min[2] + f32::from(block.z) * size);
                        }
                    }
                    println!(
                        "{path}: {} layers, {} blocks",
                        layers.layers.len(),
                        layers.layers[0].blocks.len()
                    );
                }
            }
            count += 1;
        }
    }
    assert!(count > 0, "no smap files in {dir}");
}
