//! Black box tests for the topo reader and writer.
//!
//! Game files can't be committed, the inputs are assembled by hand from the layout of the game's
//! topos. `topo_scan_dir_test` reads the game's files.

use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use arma_file_formats::{
    enfusion::{
        pak::PakReader,
        topo::{
            Areas, BuildingPoint, Buildings, Hill, PowerLine, Road, RoadType, Roads, Topo,
            TopoChunk, WaterBody,
        },
    },
    errors::{AffError, TopoError},
};

fn read(bytes: &[u8]) -> Result<Topo, AffError> {
    Topo::from(&mut Cursor::new(bytes))
}

fn write(topo: &Topo) -> Vec<u8> {
    let mut out = Vec::new();
    topo.write(&mut out).unwrap();
    out
}

fn topo_error(result: Result<impl std::fmt::Debug, AffError>) -> TopoError {
    match result {
        Err(AffError::TopoError(e)) => e,
        other => panic!("expected a TopoError, got {other:?}"),
    }
}

/// Little endian contents.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn u32(mut self, v: u32) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }

    fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }

    fn f32s(mut self, values: &[f32]) -> Self {
        values.iter().for_each(|v| self.0.extend(v.to_le_bytes()));
        self
    }

    /// A list of points.
    fn points(self, points: &[[f32; 2]]) -> Self {
        self.u32(points.len() as u32).f32s(points.as_flattened())
    }
}

/// A `TOPO` file of the chunks, with `end` offsets counted from the start of the file.
fn topo(chunks: &[(&[u8; 4], u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0; 20];
    for (tag, version, data) in chunks {
        let end = out.len() + 20 + data.len();
        out.extend(*tag);
        for v in [*version, data.len() as u32, end as u32, 0] {
            out.extend(v.to_le_bytes());
        }
        out.extend(data);
    }
    let size = out.len() - 20;
    let mut header = b"TOPO".to_vec();
    for v in [1, size as u32, out.len() as u32, 0] {
        header.extend(v.to_le_bytes());
    }
    out[..20].copy_from_slice(&header);
    out.extend(b"_EOF");
    out
}

const QUAD: [[f32; 2]; 8] = [
    [0.0, 0.0],
    [4.0, 0.0],
    [4.0, 10.0],
    [0.0, 10.0],
    [0.0, 10.0],
    [4.0, 10.0],
    [4.0, 20.0],
    [0.0, 20.0],
];

/// Two levels of detail with one minor road, two building categories, a lake and a hill.
fn chunks() -> Vec<(&'static [u8; 4], u32, Vec<u8>)> {
    let road = |vertices: &[[f32; 2]], unknown: &[u32]| {
        let mut bytes = Bytes::default()
            .u8(2)
            .points(vertices)
            .u32(unknown.len() as u32);
        for &v in unknown {
            bytes = bytes.u32(v);
        }
        bytes.0
    };
    let roads = Bytes::default()
        .u32(0)
        .u32(0)
        .u32(1)
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(2)
        .u32(1);
    let roads = [
        roads.0,
        road(&QUAD, &[4]),
        1u32.to_le_bytes().to_vec(),
        road(&QUAD[..4], &[]),
    ]
    .concat();
    let line = [[1.0, 1.0], [50.0, 1.0]];
    let power_lines = Bytes::default().u32(1).u32(1).u32(2).points(&line);
    let buildings = Bytes::default()
        .u32(2)
        .u32(1)
        .points(&QUAD[..4])
        .u32(1)
        .u32(0)
        .f32s(&[2.0, 5.0, 0.0, 1.0])
        .u32(0)
        .u32(0);
    let areas = Bytes::default()
        .u32(1)
        .u32(1)
        .points(&QUAD[..3])
        .u32(0)
        .points(&[[7.0, 8.0]]);
    let triangle = [[10.0, 10.0], [20.0, 10.0], [10.0, 20.0]];
    let water = Bytes::default()
        .u32(1)
        .u32(1)
        .u32(1)
        .u8(0)
        .points(&triangle)
        .u32(3)
        .u32(0)
        .u32(1)
        .u32(2);
    let hills = Bytes::default()
        .u32(1)
        .f32s(&[0.0, 58.0, 100.0, 200.0, 0.0, 0.0]);
    vec![
        (b"ROAD", 2, roads),
        (b"PWLN", 1, power_lines.0),
        (b"BULD", 1, buildings.0),
        (b"AREA", 1, areas.0),
        (b"WATR", 1, water.0),
        (b"HILL", 0, hills.0),
    ]
}

fn expected() -> Topo {
    let quad = QUAD.to_vec();
    Topo {
        roads: Roads {
            type_counts: [0, 0, 1, 0, 0, 0, 0, 0],
            lods: vec![
                vec![Road {
                    road_type: 2,
                    vertices: quad.clone(),
                    unknown: vec![4],
                }],
                vec![Road {
                    road_type: 2,
                    vertices: quad[..4].to_vec(),
                    unknown: vec![],
                }],
            ],
        },
        power_lines: vec![vec![PowerLine {
            power_line_type: 2,
            points: vec![[1.0, 1.0], [50.0, 1.0]],
        }]],
        buildings: vec![
            Buildings {
                footprints: vec![quad[..4].to_vec()],
                points: vec![BuildingPoint {
                    unknown: 0,
                    position: [2.0, 5.0],
                    direction: [0.0, 1.0],
                }],
            },
            Buildings::default(),
        ],
        areas: Areas {
            trees: vec![vec![quad[..3].to_vec()]],
            other: vec![],
            points: vec![[7.0, 8.0]],
        },
        water: vec![vec![WaterBody {
            water_type: 1,
            unknown: 0,
            vertices: vec![[10.0, 10.0], [20.0, 10.0], [10.0, 20.0]],
            indices: vec![0, 1, 2],
        }]],
        hills: Some(vec![Hill {
            unknown_0: 0.0,
            height: 58.0,
            position: [100.0, 200.0],
            unknown_16: [0.0, 0.0],
        }]),
        unknown_chunks: vec![],
    }
}

#[test]
fn reads_and_writes_all_chunks() {
    let bytes = topo(&chunks());
    let topo = read(&bytes).unwrap();
    assert_eq!(topo, expected());
    assert_eq!(write(&topo), bytes);
    assert_eq!(topo.roads.lods[0][0].kind(), Some(RoadType::RoadMinor));
}

#[test]
fn hills_empty_chunk_or_none() {
    let mut chunks = chunks();
    chunks[5].2.clear();
    let bytes = topo(&chunks);
    let empty = read(&bytes).unwrap();
    assert_eq!(empty.hills, None);
    assert_eq!(write(&empty), bytes);

    chunks[5].2 = 0u32.to_le_bytes().to_vec();
    let bytes = topo(&chunks);
    let zero = read(&bytes).unwrap();
    assert_eq!(zero.hills, Some(vec![]));
    assert_eq!(write(&zero), bytes);
}

#[test]
fn keeps_unknown_chunks() {
    let mut chunks = chunks();
    chunks.insert(2, (b"NEWC", 7, vec![1, 2, 3]));
    let topo = read(&topo(&chunks)).unwrap();
    assert_eq!(
        topo.unknown_chunks,
        [TopoChunk {
            tag: *b"NEWC",
            version: 7,
            data: vec![1, 2, 3],
        }]
    );
    // Written after the known chunks.
    let new = chunks.remove(2);
    chunks.push(new);
    assert_eq!(write(&topo), self::topo(&chunks));
}

#[test]
fn road_types() {
    let road = |road_type| Road {
        road_type,
        ..Road::default()
    };
    assert_eq!(road(0).kind(), Some(RoadType::Runway));
    assert_eq!(road(6).kind(), Some(RoadType::Bridge));
    assert_eq!(road(7).kind(), Some(RoadType::NotARoad));
    assert_eq!(road(8).kind(), None);
}

#[test]
fn errors() {
    assert_eq!(
        topo_error(read(b"FORM")),
        TopoError::UnknownMagic(b"FORM".to_vec())
    );

    let valid = topo(&chunks());
    assert_eq!(
        topo_error(read(&valid[..valid.len() - 5])),
        TopoError::InvalidHeader(0)
    );
    let mut no_eof = valid.clone();
    no_eof.truncate(valid.len() - 1);
    no_eof.push(b'X');
    assert!(matches!(topo_error(read(&no_eof)), TopoError::Truncated(_)));

    let mut bad_end = valid.clone();
    bad_end[20 + 12] ^= 1;
    assert_eq!(topo_error(read(&bad_end)), TopoError::InvalidHeader(20));

    let mut bad_version = valid;
    bad_version[4] = 2;
    assert_eq!(
        topo_error(read(&bad_version)),
        TopoError::UnsupportedVersion {
            chunk: "TOPO",
            version: 2
        }
    );

    let mut chunks = chunks();
    chunks[1].1 = 2;
    assert_eq!(
        topo_error(read(&topo(&chunks))),
        TopoError::UnsupportedVersion {
            chunk: "PWLN",
            version: 2
        }
    );

    let mut chunks = self::chunks();
    chunks.remove(4);
    assert_eq!(
        topo_error(read(&topo(&chunks))),
        TopoError::ChunkMissing("WATR")
    );

    // Contents left over, cut short, or a count larger than the chunk.
    for (index, name) in [(0, "ROAD"), (3, "AREA")] {
        let mut chunks = self::chunks();
        chunks[index].2.push(0);
        assert_eq!(
            topo_error(read(&topo(&chunks))),
            TopoError::InvalidChunk(name)
        );
        let len = chunks[index].2.len();
        chunks[index].2.truncate(len - 2);
        assert_eq!(
            topo_error(read(&topo(&chunks))),
            TopoError::InvalidChunk(name)
        );
    }
    let mut chunks = self::chunks();
    chunks[1].2 = Bytes::default().u32(u32::MAX).0;
    assert_eq!(
        topo_error(read(&topo(&chunks))),
        TopoError::InvalidChunk("PWLN")
    );
}

/// Reads every `.topo` in the paks below `AFF_TOPO_SCAN_DIR`, e.g. the game's `addons`
/// directory: checks that it writes back identically and the structure the docs describe.
#[test]
fn topo_scan_dir_test() {
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

    let Ok(dir) = std::env::var("AFF_TOPO_SCAN_DIR") else {
        return;
    };

    let mut paks = Vec::new();
    visit(Path::new(&dir), &mut paks);
    assert!(!paks.is_empty(), "no pak files in {dir}");

    let mut count = 0;
    for pak in paks {
        let reader = PakReader::from_stream(BufReader::new(File::open(&pak).unwrap())).unwrap();
        for entry in &reader.pak.entries {
            if !entry.path.ends_with(".topo") {
                continue;
            }
            let path = &entry.path;
            let bytes = reader.read_entry(entry).unwrap();
            let topo = read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert_eq!(write(&topo), bytes, "{path}");
            assert!(topo.unknown_chunks.is_empty(), "{path}");

            let lods = &topo.roads.lods;
            for lod in lods {
                assert_eq!(lod.len(), lods[0].len(), "{path}");
                for road in lod {
                    assert!(road.kind().is_some(), "{path}");
                    assert_eq!(road.vertices.len() % 4, 0, "{path}");
                    assert!(
                        road.unknown
                            .iter()
                            .all(|&v| v % 4 == 0 && (v as usize) < road.vertices.len()),
                        "{path}"
                    );
                }
            }
            for (a, b) in lods[0].iter().zip(&lods[lods.len() - 1]) {
                assert_eq!(a.road_type, b.road_type, "{path}");
            }
            assert_eq!(topo.buildings.len(), 5, "{path}");
            for body in topo.water.iter().flatten() {
                assert!(body.water_type < 4, "{path}");
                assert_eq!(body.indices.len() % 3, 0, "{path}");
                assert!(
                    body.indices
                        .iter()
                        .all(|&i| (i as usize) < body.vertices.len()),
                    "{path}"
                );
            }
            for point in &topo.buildings[0].points {
                let [cos, sin] = point.direction;
                assert!((0.85..1.15).contains(&cos.hypot(sin)), "{path}");
            }

            println!(
                "{path}: {} roads, {} buildings, {} water bodies, {} area points, {:?} hills",
                lods[0].len(),
                topo.buildings[0].footprints.len(),
                topo.water[0].len(),
                topo.areas.points.len(),
                topo.hills.as_ref().map(Vec::len)
            );
            count += 1;
        }
    }
    assert!(count > 0, "no topo files in {dir}");
}
