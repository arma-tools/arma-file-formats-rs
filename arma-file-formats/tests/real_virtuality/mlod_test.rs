//! Tests for the MLOD `.p3d` reader and writer.
//!
//! `test-data/mlod_in/` holds `NewGeometryTemplate.p3d` of the Arma 3 Tools
//! (`ObjectBuilder\O2Scripts\CharacterGeo`, 14 LODs, quads, properties, mass, sharp edges,
//! proxies) and two models saved by Object Builder's `O2Script.exe`: `o2script_all.p3d` has
//! every kind of tagg, `o2script_weights.p3d` a selection of 300 points weighted `i / 300`.

use std::io::Cursor;

use arma_file_formats::{
    errors::{AffError, MlodError},
    real_virtuality::p3d::{
        Mlod, MlodFace, MlodLod, MlodPoint, MlodSelection, MlodTagg, MlodVertex, weight_from_byte,
        weight_to_byte,
    },
};

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/mlod_in/";

const FILES: [&str; 3] = [
    "NewGeometryTemplate.p3d",
    "o2script_all.p3d",
    "o2script_weights.p3d",
];

fn input(name: &str) -> Vec<u8> {
    std::fs::read(format!("{INPUT_PATH_PREFIX}{name}")).unwrap()
}

fn read(bytes: &[u8]) -> Result<Mlod, AffError> {
    Mlod::from(&mut Cursor::new(bytes))
}

fn write(mlod: &Mlod) -> Result<Vec<u8>, AffError> {
    let mut out = Vec::new();
    mlod.write(&mut out)?;
    Ok(out)
}

fn error<T: std::fmt::Debug>(result: Result<T, AffError>) -> MlodError {
    match result {
        Err(AffError::MlodError(e)) => e,
        other => panic!("expected an MlodError, got {other:?}"),
    }
}

#[test]
fn mlod_round_trip_test() {
    for name in FILES {
        let bytes = input(name);
        assert_eq!(write(&read(&bytes).unwrap()).unwrap(), bytes, "{name}");
    }
    let path = format!("{INPUT_PATH_PREFIX}o2script_all.p3d");
    assert_eq!(
        Mlod::from_path(path).unwrap(),
        read(&input("o2script_all.p3d")).unwrap()
    );
}

#[test]
fn mlod_template_test() {
    let mlod = read(&input("NewGeometryTemplate.p3d")).unwrap();
    let resolutions: Vec<f32> = mlod.lods.iter().map(|l| l.resolution).collect();
    assert_eq!(
        resolutions,
        [
            1.0, 2.0, 3.0, 4.0, 5.0, 1.1e3, 1e4, 1.001e4, 1e13, 1e15, 2e15, 5e15, 6e15, 7e15
        ]
    );
    // Empty LODs still have the UV set 0.
    let empty = mlod.lod(1e15).unwrap();
    assert!(empty.points.is_empty() && empty.faces.is_empty());
    assert_eq!(empty.uv_set(0), Some(&[][..]));
    assert_eq!(mlod.lod(7e15).unwrap().uv_set(1), Some(&[][..]));

    let geometry = mlod.lod(1e13).unwrap();
    assert_eq!(
        (
            geometry.points.len(),
            geometry.normals.len(),
            geometry.faces.len()
        ),
        (42, 42, 54)
    );
    assert_eq!(geometry.flags, 0);
    let properties: Vec<_> = geometry.properties().collect();
    assert_eq!(
        properties,
        [
            ("class", "man"),
            ("prefershadowvolume", "1"),
            ("reversed", "1")
        ]
    );
    assert_eq!(geometry.property("CLASS"), Some("man"));
    assert_eq!(geometry.property("map"), None);
    let mass: f32 = geometry.masses().unwrap().iter().sum();
    assert!((mass - 87.5).abs() < 1e-3);
    assert_eq!(geometry.named_selections().count(), 44);
    assert!(geometry.named_selection("component01").is_some());
    let quads = geometry.faces.iter().filter(|f| f.sides == 4).count();
    assert_eq!(geometry.corners(), 54 * 3 + quads);
    assert_eq!(geometry.uv_set(0).unwrap().len(), geometry.corners());

    let view = mlod.lod(6e15).unwrap();
    assert_eq!(view.sharp_edges().unwrap().len(), 268);
    let proxy = view
        .named_selection("proxy:\\A3\\Characters_F\\Proxies\\weapon.001")
        .unwrap();
    assert_eq!(proxy.faces.iter().filter(|&&f| f != 0).count(), 1);
    assert_eq!(proxy.points.iter().filter(|&&p| p != 0).count(), 3);
    // A point flagged hidden.
    assert!(
        mlod.lods
            .iter()
            .flat_map(|l| &l.points)
            .any(|p| p.flags == 0x0100_0000)
    );
}

#[test]
fn mlod_o2script_test() {
    let mlod = read(&input("o2script_all.p3d")).unwrap();
    assert_eq!(mlod.lods.len(), 2);
    let lod = &mlod.lods[0];
    assert_eq!(lod.resolution, 1.5);
    assert_eq!(
        lod.points[4],
        MlodPoint {
            position: [0.5, 0.5, 1.0],
            // `POINT_FULLLIGHT` at bits 4 to 7.
            flags: 0x40,
        }
    );
    assert_eq!(lod.normals.len(), 7);
    let quad = &lod.faces[0];
    assert_eq!(quad.sides, 4);
    assert_eq!(quad.texture, "a3\\data_f\\white_ca.paa");
    assert_eq!(quad.material, "a3\\data_f\\default.rvmat");
    assert_eq!(
        quad.used()[2],
        MlodVertex {
            point: 2,
            normal: 2,
            u: 1.0,
            v: 1.0
        }
    );
    let triangle = &lod.faces[1];
    assert_eq!(triangle.used().len(), 3);
    assert_eq!(triangle.vertices[3], MlodVertex::default());
    assert_eq!(triangle.texture, "tri_co.paa");
    assert_eq!(triangle.material, "");
    // Z bias 2 at bits 8 and 9.
    assert_eq!(triangle.flags, 0x200);

    assert_eq!(lod.sharp_edges(), Some(&[[0, 1], [1, 2]][..]));
    let selection = |points: [u8; 5], faces: [u8; 2]| MlodSelection {
        points: points.to_vec(),
        faces: faces.to_vec(),
    };
    assert_eq!(
        lod.taggs[1],
        MlodTagg::Selected(selection([0, 1, 0, 0, 0], [1, 0]))
    );
    let bone = lod.named_selection("bone").unwrap();
    assert_eq!(bone, &selection([1, 0, 0, 0, 128], [0, 1]));
    assert_eq!(bone.weight(0), 1.0);
    assert_eq!(bone.weight(1), 0.0);
    assert_eq!(bone.weight(4), 128.0 / 255.0);
    assert_eq!(bone.weight(5), 0.0);
    assert!(bone.has_face(1) && !bone.has_face(0) && !bone.has_face(2));
    assert_eq!(
        lod.taggs[5],
        MlodTagg::Lock(selection([0, 0, 0, 1, 0], [0, 0]))
    );
    assert_eq!(
        lod.taggs[6],
        MlodTagg::Hidden(selection([0, 0, 1, 0, 0], [0, 0]))
    );
    let MlodTagg::Animation { time, positions } = &lod.taggs[7] else {
        panic!("expected an animation, got {:?}", lod.taggs[7]);
    };
    assert_eq!(*time, 0.5);
    assert_eq!(positions[4], [0.5, 0.5, 2.0]);
    assert_eq!(positions[1], lod.points[1].position);
    assert_eq!(
        lod.uv_set(0).unwrap()[..4],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
    );
    assert_eq!(
        lod.uv_set(1).unwrap()[4..],
        [[0.25, 0.25], [0.75, 0.25], [0.5, 0.75]]
    );
    assert_eq!(lod.uv_set(2), None);
    assert_eq!(lod.masses(), None);

    let geometry = mlod.lod(1e13).unwrap();
    assert_eq!(geometry.masses(), Some(&[10.0, 0.0, 0.0, 2.5][..]));
    assert_eq!(mlod.lod(2.0), None);
}

#[test]
fn mlod_weights_test() {
    // Object Builder's bytes for the weights `i / 300`.
    let mlod = read(&input("o2script_weights.p3d")).unwrap();
    let selection = mlod.lods[0].named_selection("w").unwrap();
    assert_eq!(selection.points.len(), 301);
    assert_eq!(selection.points[0], 0);
    for i in 1..=300 {
        let weight = i as f32 / 300.0;
        assert_eq!(weight_to_byte(weight), selection.points[i], "{i}");
        assert!(
            (selection.weight(i) - weight).abs() <= 0.5 / 255.0 + 1e-6,
            "{i}"
        );
    }
    // Probed with O2Script as well: rounded half to even, clamped.
    for (weight, byte) in [
        (0.0005, 0),
        (0.00196, 0),
        (0.00197, 255),
        (0.5 / 255.0, 0),
        (1.5 / 255.0, 254),
        (2.5 / 255.0, 254),
        (0.25, 192),
        (0.5, 128),
        (0.75, 65),
        (0.9999, 1),
        (1.5, 1),
        (-0.5, 0),
        (f32::NAN, 0),
    ] {
        assert_eq!(weight_to_byte(weight), byte, "{weight}");
    }
    assert_eq!(weight_from_byte(0), 0.0);
    assert_eq!(weight_from_byte(1), 1.0);
    assert_eq!(weight_from_byte(255), 1.0 / 255.0);
    for byte in 0..=255 {
        assert_eq!(weight_to_byte(weight_from_byte(byte)), byte);
    }
}

fn triangle_lod() -> MlodLod {
    let vertex = |point| MlodVertex {
        point,
        normal: 0,
        u: 0.0,
        v: 0.0,
    };
    MlodLod {
        resolution: 1.0,
        flags: 0,
        points: vec![MlodPoint::default(); 3],
        normals: vec![[0.0, 1.0, 0.0]],
        faces: vec![MlodFace {
            sides: 3,
            vertices: [vertex(0), vertex(1), vertex(2), MlodVertex::default()],
            flags: 0,
            texture: "t.paa".into(),
            material: String::new(),
        }],
        taggs: vec![
            MlodTagg::NamedSelection {
                name: "all".into(),
                selection: MlodSelection {
                    points: vec![1; 3],
                    faces: vec![1],
                },
            },
            MlodTagg::UvSet {
                id: 0,
                uvs: vec![[0.0, 0.0]; 3],
            },
        ],
    }
}

#[test]
fn mlod_build_test() {
    let mlod = Mlod {
        lods: vec![triangle_lod()],
    };
    let bytes = write(&mlod).unwrap();
    assert_eq!(&bytes[..12], b"MLOD\x01\x01\0\0\x01\0\0\0");
    assert_eq!(&bytes[12..16], b"P3DM");
    assert!(bytes.ends_with(b"\x01#EndOfFile#\0\0\0\0\0\0\0\x80\x3f"));
    assert_eq!(read(&bytes).unwrap(), mlod);
    assert_eq!(
        write(&Mlod::default()).unwrap(),
        b"MLOD\x01\x01\0\0\0\0\0\0"
    );

    // Unknown taggs and ones with another first byte are kept.
    let mut lod = triangle_lod();
    lod.taggs.push(MlodTagg::Unknown {
        active: 1,
        name: "#Custom#".into(),
        data: vec![1, 2, 3],
    });
    lod.taggs.push(MlodTagg::Unknown {
        active: 0,
        name: "#Mass#".into(),
        data: vec![],
    });
    let mlod = Mlod { lods: vec![lod] };
    assert_eq!(read(&write(&mlod).unwrap()).unwrap(), mlod);
}

#[test]
fn mlod_write_error_test() {
    let check = |change: fn(&mut MlodLod), expected: MlodError| {
        let mut lod = triangle_lod();
        change(&mut lod);
        assert_eq!(error(write(&Mlod { lods: vec![lod] })), expected);
    };
    check(
        |l| l.taggs[0] = MlodTagg::Selected(MlodSelection::default()),
        MlodError::InvalidSelection {
            name: "#Selected#".into(),
            points: 0,
            faces: 0,
        },
    );
    check(
        |l| {
            l.taggs[0] = MlodTagg::NamedSelection {
                name: "#x".into(),
                selection: MlodSelection::default(),
            }
        },
        MlodError::InvalidSelectionName("#x".into()),
    );
    check(
        |l| l.taggs.push(MlodTagg::Mass(vec![1.0])),
        MlodError::InvalidTaggLength {
            name: "#Mass#".into(),
            len: 1,
            expected: 3,
        },
    );
    check(
        |l| l.taggs[1] = MlodTagg::UvSet { id: 1, uvs: vec![] },
        MlodError::InvalidTaggLength {
            name: "#UVSet#".into(),
            len: 0,
            expected: 3,
        },
    );
    check(
        |l| {
            l.taggs.push(MlodTagg::Animation {
                time: 0.0,
                positions: vec![],
            })
        },
        MlodError::InvalidTaggLength {
            name: "#Animation#".into(),
            len: 0,
            expected: 3,
        },
    );
    check(
        |l| {
            l.taggs.push(MlodTagg::Property {
                name: "x".repeat(64),
                value: String::new(),
            })
        },
        MlodError::InvalidProperty("x".repeat(64)),
    );
    check(
        |l| l.faces[0].texture = "a\0b".into(),
        MlodError::InvalidName("a\0b".into()),
    );
    check(
        |l| {
            l.taggs.push(MlodTagg::Unknown {
                active: 1,
                name: "#EndOfFile#".into(),
                data: vec![],
            })
        },
        MlodError::EndOfFileTagg,
    );
}

#[test]
fn mlod_read_error_test() {
    let bytes = input("o2script_all.p3d");
    let with = |at: usize, patch: &[u8]| {
        let mut b = bytes.clone();
        b[at..at + patch.len()].copy_from_slice(patch);
        error(read(&b))
    };
    assert_eq!(with(0, b"ODOL"), MlodError::UnknownSignature("ODOL".into()));
    assert_eq!(
        with(4, &0x102_u32.to_le_bytes()),
        MlodError::UnsupportedVersion(0x102)
    );
    assert_eq!(
        with(12, b"SP3X"),
        MlodError::UnknownLodSignature("SP3X".into())
    );
    assert_eq!(
        with(16, &0x1b_u32.to_le_bytes()),
        MlodError::UnsupportedLodVersion {
            major: 0x1b,
            minor: 0x100
        }
    );
    // The first face starts after 5 points and 7 normals.
    assert_eq!(
        with(12 + 28 + 5 * 16 + 7 * 12, &5_u32.to_le_bytes()),
        MlodError::InvalidSides(5)
    );
    for len in [0, 3, 11, 100, 1000, bytes.len() - 1] {
        assert_eq!(error(read(&bytes[..len])), MlodError::Truncated, "{len}");
    }
    let mut longer = bytes.clone();
    longer.push(0);
    assert_eq!(error(read(&longer)), MlodError::TrailingData(1));
    // Two LODs announced, three read.
    assert_eq!(with(8, &3_u32.to_le_bytes()), MlodError::Truncated);

    let at = bytes.windows(4).position(|w| w == b"TAGG").unwrap();
    assert_eq!(with(at, b"GGAT"), MlodError::TaggsMissing);
    // `#Selected#` of 5 points and 2 faces made 1 byte longer.
    let at = bytes.windows(10).position(|w| w == b"#Selected#").unwrap() + 11;
    assert_eq!(&bytes[at..at + 4], &7_u32.to_le_bytes());
    assert_eq!(
        with(at, &8_u32.to_le_bytes()),
        MlodError::InvalidTaggSize {
            name: "#Selected#".into(),
            size: 8,
            expected: 7
        }
    );
    let at = bytes.windows(10).position(|w| w == b"tri_co.paa").unwrap();
    assert_eq!(
        with(at, b"\xff"),
        MlodError::InvalidString(b"\xffri_co.paa".to_vec())
    );
}

#[test]
fn mlod_uv_set_size_test() {
    // Object Builder ignores the size of `#UVSet#`: a few mod models store 4 less, without the
    // id. Written back with the right size.
    let bytes = input("o2script_all.p3d");
    let at = bytes.windows(7).position(|w| w == b"#UVSet#").unwrap() + 8;
    assert_eq!(&bytes[at..at + 4], &60_u32.to_le_bytes());
    for size in [0_u32, 56, 1000] {
        let mut short = bytes.clone();
        short[at..at + 4].copy_from_slice(&size.to_le_bytes());
        let mlod = read(&short).unwrap();
        assert_eq!(mlod, read(&bytes).unwrap(), "{size}");
        assert_eq!(write(&mlod).unwrap(), bytes, "{size}");
    }
}

#[test]
fn mlod_raw_tagg_test() {
    let bytes = input("o2script_all.p3d");
    let patched = |at: usize, patch: &[u8]| {
        let mut b = bytes.clone();
        b[at..at + patch.len()].copy_from_slice(patch);
        b
    };
    let tagg = |name: &[u8]| bytes.windows(name.len()).position(|w| w == name).unwrap();

    // Bytes after the NUL of a property value are kept as an unknown tagg.
    let at = tagg(b"#Property#") + 15;
    let b = patched(at + 64 + 10, b"x");
    let mlod = read(&b).unwrap();
    let MlodTagg::Unknown { active, name, data } = &mlod.lods[0].taggs[3] else {
        panic!("expected an unknown tagg, got {:?}", mlod.lods[0].taggs[3]);
    };
    assert_eq!((*active, name.as_str(), data.len()), (1, "#Property#", 128));
    assert_eq!(&data[..5], b"class");
    assert_eq!(mlod.lods[0].properties().count(), 1);
    assert_eq!(write(&mlod).unwrap(), b);

    // And without a NUL.
    let b = patched(at, &[b'x'; 64]);
    let mlod = read(&b).unwrap();
    assert!(
        matches!(&mlod.lods[0].taggs[3], MlodTagg::Unknown { name, .. } if name == "#Property#")
    );
    assert_eq!(write(&mlod).unwrap(), b);

    // A tagg with another first byte is kept as it is.
    let b = patched(tagg(b"#Selected#") - 1, &[0]);
    let mlod = read(&b).unwrap();
    assert!(matches!(
        &mlod.lods[0].taggs[1],
        MlodTagg::Unknown { active: 0, name, data } if name == "#Selected#" && data.len() == 7
    ));
    assert_eq!(write(&mlod).unwrap(), b);

    // Sharp edges are pairs of `u32`.
    let size = tagg(b"#SharpEdges#") + 13;
    assert_eq!(&bytes[size..size + 4], &16_u32.to_le_bytes());
    assert_eq!(
        error(read(&patched(size, &15_u32.to_le_bytes()))),
        MlodError::InvalidTaggSize {
            name: "#SharpEdges#".into(),
            size: 15,
            expected: 8
        }
    );
}
