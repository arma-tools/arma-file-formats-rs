//! Black box tests for the anm reader and writer.
//!
//! The probes in `anm_probes/` were built by Workbench from the `.txa` next to them, see
//! `anm_probes/README.md`. The expected values below are the `.txa` values. `anm_scan_dir_test`
//! reads the game's files.

use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use arma_file_formats::{
    enfusion::{
        anm::{Anm, AnmBone, AnmEvents, AnmKey, AnmPlainEvent, AnmTrack, AnmVersion},
        ent::{Entry, Value},
        pak::PakReader,
    },
    errors::{AffError, AnmError},
};

const PROBES: &[&str] = &[
    "p_t",
    "p_tneg",
    "p_full5",
    "p_q1",
    "p_tqs",
    "p_two",
    "p_qs",
    "p_diff",
    "p_ev1",
    "p_cp",
    "p_linear",
    "p_hold",
    "p_slerp",
    "p_shortarc",
    "p_one",
];

fn probe_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/enfusion/anm_probes")
        .join(format!("{name}.anm"));
    std::fs::read(path).unwrap()
}

fn read(bytes: &[u8]) -> Result<Anm, AffError> {
    Anm::from(&mut Cursor::new(bytes))
}

fn probe(name: &str) -> Anm {
    read(&probe_bytes(name)).unwrap()
}

fn write(anm: &Anm) -> Vec<u8> {
    let mut out = Vec::new();
    anm.write(&mut out).unwrap();
    out
}

fn anm_error(result: Result<impl std::fmt::Debug, AffError>) -> AnmError {
    match result {
        Err(AffError::AnmError(e)) => e,
        other => panic!("expected an AnmError, got {other:?}"),
    }
}

/// Frames and values of a track, compared to the expected ones within half a step, the
/// rounding of SET7.
fn assert_values<const N: usize>(track: &AnmTrack<N>, expected: &[(u16, [f32; N])]) {
    assert_values_within(track, expected, 0.5);
}

fn assert_values_within<const N: usize>(
    track: &AnmTrack<N>,
    expected: &[(u16, [f32; N])],
    steps: f32,
) {
    let values: Vec<_> = track.values().collect();
    assert_eq!(values.len(), expected.len(), "{values:?}");
    for ((frame, value), (expected_frame, expected_value)) in values.iter().zip(expected) {
        assert_eq!(frame, expected_frame, "{values:?}");
        for (v, e) in value.iter().zip(expected_value) {
            assert!((v - e).abs() <= track.step * steps + 1e-6, "{values:?}");
        }
    }
}

/// `0.7071068`, the value written in the TXAs. Not `FRAC_1_SQRT_2`, which differs in the last bit.
const R: f32 = f32::from_bits(0x3f35_04f4);
const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[test]
fn anm_probes_round_trip_test() {
    for name in PROBES {
        let bytes = probe_bytes(name);
        let anm = read(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(write(&anm), bytes, "{name}");
        assert!(anm.unknown_chunks.is_empty(), "{name}");
    }
}

#[test]
fn anm_translation_test() {
    let anm = probe("p_t");
    assert_eq!((anm.frames, anm.fps), (2, 30));
    assert_eq!(anm.duration(), 1.0 / 30.0);
    assert_eq!(anm.bones.len(), 1);
    let bone = &anm.bones[0];
    assert_eq!(bone.name, "b");
    assert!(!bone.additive);
    assert_eq!(bone.unknown_flags, 0);

    // One range for all components: 0..3.
    assert_eq!(bone.translation.min, 0.0);
    assert_eq!(bone.translation.step, 3.0 / 65535.0);
    assert_eq!(
        bone.translation.keys,
        [
            AnmKey {
                frame: 0,
                value: [0, 0, 0]
            },
            AnmKey {
                frame: 1,
                value: [0x5555, 0xaaaa, 0xffff]
            },
        ]
    );
    // Not keyed: one identity rotation key and no scale keys.
    assert_values(&bone.rotation, &[(0, IDENTITY)]);
    assert!(bone.scale.keys.is_empty());
    assert_eq!((bone.scale.min, bone.scale.step), (1.0, 0.0));

    assert_values(
        &probe("p_tneg").bones[0].translation,
        &[(0, [-1.0, 0.5, 0.0]), (1, [0.0; 3]), (2, [2.0, -0.5, 1.0])],
    );
}

#[test]
fn anm_full_track_test() {
    let anm = probe("p_full5");
    let expected: Vec<_> = (0..5u16)
        .map(|f| {
            let f32_frame = f32::from(f);
            (
                f,
                [
                    f32_frame * 0.25,
                    1.0 - f32_frame * 0.25,
                    f32::from(f % 2) * 0.5,
                ],
            )
        })
        .collect();
    assert_values(&anm.bones[0].translation, &expected);
}

#[test]
fn anm_all_tracks_test() {
    let anm = probe("p_tqs");
    let bone = &anm.bones[0];
    assert_values(
        &bone.translation,
        &[(0, [0.0; 3]), (1, [1.0, 2.0, 3.0]), (2, [3.0, 2.0, 1.0])],
    );
    assert_values(
        &bone.rotation,
        &[(0, IDENTITY), (1, [0.0, R, 0.0, R]), (2, [R, 0.0, 0.0, R])],
    );
    assert_values(
        &bone.scale,
        &[(0, [1.0; 3]), (1, [2.0; 3]), (2, [1.0, 2.0, 4.0])],
    );
}

#[test]
fn anm_constant_rotation_test() {
    let anm = probe("p_q1");
    let rotation = &anm.bones[0].rotation;
    assert_eq!(rotation.min, 0.0);
    assert_eq!(rotation.step, R / 65535.0);
    assert_values(rotation, &[(0, [0.0, R, 0.0, R])]);
}

#[test]
fn anm_bones_test() {
    let anm = probe("p_two");
    let names: Vec<_> = anm.bones.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["a", "c"]);
    assert_eq!(anm.bone("C").map(|b| b.name.as_str()), Some("c"));
    assert!(anm.bone("b").is_none());
    assert_values(
        &anm.bone("c").unwrap().translation,
        &[(0, [0.0; 3]), (1, [0.0, 0.0, 2.0])],
    );
}

#[test]
fn anm_additive_test() {
    let turn = [(0, IDENTITY), (1, [0.0, R, 0.0, R])];
    for (name, additive) in [("p_qs", false), ("p_diff", true)] {
        let anm = probe(name);
        let bone = &anm.bones[0];
        assert_eq!(bone.additive, additive, "{name}");
        // `$keys q s` without `t`.
        assert!(bone.translation.keys.is_empty(), "{name}");
        assert_values(&bone.rotation, &turn);
    }
}

#[test]
fn anm_key_reduction_test() {
    let anm = probe("p_linear");
    assert_eq!(anm.frames, 9);
    assert_values(
        &anm.bones[0].translation,
        &[(0, [0.0; 3]), (8, [1.0, 0.0, 0.0])],
    );

    let anm = probe("p_hold");
    assert_values(
        &anm.bones[0].translation,
        &[(0, [0.0; 3]), (3, [0.0; 3]), (4, [1.0, 0.0, 0.0])],
    );

    let anm = probe("p_slerp");
    assert_values(
        &anm.bones[0].rotation,
        &[(0, IDENTITY), (8, [0.0, R, 0.0, R])],
    );
}

#[test]
fn anm_events_test() {
    let anm = probe("p_ev1");
    let Some(AnmEvents::Objects(events)) = &anm.events else {
        panic!("{:?}", anm.events)
    };
    let events: Vec<_> = events.entries.iter().filter_map(Entry::as_object).collect();
    assert_eq!(events.len(), 2);
    let string = |s: &str| Some(Value::String(s.to_string()));
    for event in &events {
        assert_eq!(
            event.key.as_deref(),
            Some("AnimSrcEventGeneric"),
            "{event:?}"
        );
    }
    assert_eq!(events[0].get("Name").cloned(), string("Ev"));
    assert_eq!(events[0].get("Frame"), Some(&Value::Int(0)));
    assert_eq!(events[0].get("UserInt"), Some(&Value::Int(7)));
    assert_eq!(events[1].get("Name").cloned(), string("EvB"));
    assert_eq!(events[1].get("Frame"), Some(&Value::Int(1)));
    assert_eq!(events[1].get("FrameCount"), Some(&Value::Int(4)));
    assert!(anm.properties.is_none());
}

#[test]
fn anm_properties_test() {
    let anm = probe("p_cp");
    let properties = anm.properties.as_ref().unwrap();
    let pairs: Vec<_> = properties
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(pairs, [("alpha", "one"), ("b", "")]);
    assert!(anm.events.is_none());
}

fn assert_close<const N: usize>(value: Option<[f32; N]>, expected: [f32; N]) {
    let value = value.unwrap();
    assert!(
        value
            .iter()
            .zip(&expected)
            .all(|(v, e)| (v - e).abs() < 1e-3),
        "{value:?} != {expected:?}"
    );
}

/// Rotation about y, `degrees * frame / 8` like the reduced probe turns.
fn turn_y(degrees: f32, frame: f32) -> [f32; 4] {
    let half = (degrees * frame / 8.0 / 2.0).to_radians();
    [0.0, half.sin(), 0.0, half.cos()]
}

#[test]
fn anm_sample_test() {
    // Linear between keys, held before and after them.
    let linear = &probe("p_linear").bones[0].translation;
    assert_close(linear.sample(2.0), [0.25, 0.0, 0.0]);
    assert_close(linear.sample(-1.0), [0.0; 3]);
    let hold = &probe("p_hold").bones[0].translation;
    assert_close(hold.sample(2.0), [0.0; 3]);
    assert_close(hold.sample(3.5), [0.5, 0.0, 0.0]);
    assert_close(hold.sample(6.0), [1.0, 0.0, 0.0]);
    assert_close(hold.sample(100.0), [1.0, 0.0, 0.0]);

    // The keys dropped by Workbench are reproduced by slerp.
    let slerp = &probe("p_slerp").bones[0].rotation;
    let shortarc = &probe("p_shortarc").bones[0].rotation;
    for frame in 0..=8u16 {
        let f = f32::from(frame);
        assert_close(slerp.sample(f), turn_y(90.0, f));
        let mut q = shortarc.sample(f).unwrap();
        // The last key is the negated quaternion of the turn.
        if frame == 8 {
            q = q.map(|v| -v);
        }
        assert_close(Some(q), turn_y(-160.0, f));
    }
    assert_close(slerp.sample(2.5), turn_y(90.0, 2.5));

    assert!(probe("p_qs").bones[0].translation.sample(0.0).is_none());
}

#[test]
fn anm_one_frame_test() {
    let anm = probe("p_one");
    assert_eq!(anm.frames, 1);
    assert_eq!(anm.duration(), 0.0);
    let bone = &anm.bones[0];
    assert_values(&bone.translation, &[(0, [1.0, 2.0, 3.0])]);
    assert_values(&bone.rotation, &[(0, [0.0, R, 0.0, R])]);
    assert_values(&bone.scale, &[(0, [2.0; 3])]);
}

#[test]
fn anm_new_bone_test() {
    // Workbench's ranges of tracks without keys: `p_qs` has no translation keys.
    let bone = AnmBone::new("b");
    let probe = probe("p_qs");
    assert_eq!(bone.translation, probe.bones[0].translation);
    assert_eq!(probe.bones[0].scale, bone.scale);
    assert_eq!(bone.rotation.step.to_bits(), 0x3780_0080);
}

#[test]
fn anm_quantize_test() {
    // The quantization of Workbench, for tracks that are not constant.
    for name in ["p_t", "p_tneg", "p_full5", "p_tqs", "p_linear"] {
        let anm = probe(name);
        let track = &anm.bones[0].translation;
        let values: Vec<_> = track.values().collect();
        let quantized = AnmTrack::quantize(&values);
        assert_eq!(quantized.min, track.min, "{name}");
        assert_eq!(quantized.step, track.step, "{name}");
        assert_eq!(quantized.keys, track.keys, "{name}");
    }

    let empty = AnmTrack::<3>::quantize(&[]);
    assert!(empty.keys.is_empty());
    let constant = AnmTrack::quantize(&[(0, [2.0, 2.0, 2.0])]);
    assert_eq!((constant.min, constant.step), (2.0, 0.0));
    assert_values(&constant, &[(0, [2.0; 3])]);
}

#[test]
fn anm_write_test() {
    let mut anm = probe("p_hold");
    anm.bones[0].translation = AnmTrack::quantize(&[(0, [0.0; 3]), (5, [4.0, 0.0, -4.0])]);
    anm.bones[0].name = "Hips".to_string();
    anm.bones[0].additive = true;
    anm.properties = Some(vec![("profile".to_string(), "test".to_string())]);
    let again = read(&write(&anm)).unwrap();
    assert_eq!(again, anm);

    // Keys on every frame have no frame numbers in the file, they must be 0, 1, 2...
    let mut anm = probe("p_full5");
    anm.bones[0].translation.keys[1].frame = 7;
    assert_eq!(
        anm_error(anm.write(&mut Vec::new())),
        AnmError::Unwritable("DATA")
    );
    // More keys than frames.
    let mut anm = probe("p_t");
    let key = anm.bones[0].translation.keys[0];
    anm.bones[0].translation.keys.push(key);
    assert_eq!(
        anm_error(anm.write(&mut Vec::new())),
        AnmError::Unwritable("DATA")
    );
    let mut anm = probe("p_t");
    anm.bones[0].name = "a\0b".to_string();
    assert_eq!(
        anm_error(anm.write(&mut Vec::new())),
        AnmError::Unwritable("HEAD")
    );
}

/// Replaces `len` bytes at `at` and fixes the sizes of `FORM`, `SET7` and the chunk at `chunk`.
fn patch(bytes: &[u8], chunk: usize, at: usize, len: usize, with: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    out.splice(at..at + len, with.iter().copied());
    let delta = with.len() as i64 - len as i64;
    for size_at in [4, 16, chunk + 4] {
        let size = u32::from_be_bytes(out[size_at..size_at + 4].try_into().unwrap());
        out[size_at..size_at + 4]
            .copy_from_slice(&((i64::from(size) + delta) as u32).to_be_bytes());
    }
    out
}

#[test]
fn anm_errors_test() {
    let bytes = probe_bytes("p_t");
    // HEAD starts at 20, DATA at 70.
    assert_eq!(&bytes[20..24], b"HEAD");
    assert_eq!(&bytes[70..74], b"DATA");

    let mut wrong = bytes.clone();
    wrong[8..12].copy_from_slice(b"XOB9");
    assert_eq!(
        anm_error(read(&wrong)),
        AnmError::UnknownFormType("XOB9".into())
    );

    let mut set5 = bytes.clone();
    set5[12..16].copy_from_slice(b"SET5");
    assert_eq!(
        anm_error(read(&set5)),
        AnmError::UnsupportedSet("SET5".into())
    );
    // SET7 chunks under a SET6 tag.
    let mut set6 = bytes.clone();
    set6[12..16].copy_from_slice(b"SET6");
    assert_eq!(anm_error(read(&set6)), AnmError::ChunkMissing("FPS"));

    let mut renamed = bytes.clone();
    renamed[70..74].copy_from_slice(b"DATX");
    assert_eq!(anm_error(read(&renamed)), AnmError::ChunkMissing("DATA"));

    // A key less in DATA.
    let short = patch(&bytes, 70, 78 + 6, 6, &[]);
    assert_eq!(anm_error(read(&short)), AnmError::InvalidChunk("DATA"));
    // Trailing bytes in DATA.
    let long = patch(&bytes, 70, 78, 0, &[0, 0]);
    assert_eq!(anm_error(read(&long)), AnmError::InvalidChunk("DATA"));

    // The name length of the bone entry doesn't match the name.
    let mut name_len = bytes.clone();
    name_len[20 + 8 + 8 + 2 + 7] = 5;
    assert_eq!(anm_error(read(&name_len)), AnmError::InvalidChunk("HEAD"));

    // Unknown chunks are kept and written after the known ones.
    let mut with_unknown = bytes.clone();
    with_unknown.extend(b"ABCD\0\0\0\x02xy");
    let size = |b: &mut Vec<u8>, at: usize, add: u32| {
        let s = u32::from_be_bytes(b[at..at + 4].try_into().unwrap()) + add;
        b[at..at + 4].copy_from_slice(&s.to_be_bytes());
    };
    size(&mut with_unknown, 4, 10);
    size(&mut with_unknown, 16, 10);
    let anm = read(&with_unknown).unwrap();
    assert_eq!(anm.unknown_chunks.len(), 1);
    assert_eq!(&anm.unknown_chunks[0].tag, b"ABCD");
    assert_eq!(write(&anm), with_unknown);
}

/// Reads every `.anm` in the paks below `AFF_ANM_SCAN_DIR`, e.g. the game's `addons` directory:
/// `AFF_ANM_SCAN_DIR=".../Arma Reforger/addons" cargo test --release anm_scan_dir_test`.
#[test]
fn anm_scan_dir_test() {
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

    let Ok(dir) = std::env::var("AFF_ANM_SCAN_DIR") else {
        return;
    };

    let mut paks = Vec::new();
    visit(Path::new(&dir), &mut paks);
    assert!(!paks.is_empty(), "no pak files in {dir}");

    let mut count = 0;
    for pak in paks {
        let reader = PakReader::from_stream(BufReader::new(File::open(&pak).unwrap())).unwrap();
        for entry in &reader.pak.entries {
            if !entry.path.to_ascii_lowercase().ends_with(".anm") {
                continue;
            }
            let path = &entry.path;
            let bytes = reader.read_entry(entry).unwrap();
            let anm = read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert_eq!(write(&anm), bytes, "{path}");
            assert!(anm.unknown_chunks.is_empty(), "{path}");
            for bone in &anm.bones {
                assert_eq!(bone.unknown_flags, 0, "{path} {}", bone.name);
                for frames in [
                    bone.translation
                        .keys
                        .iter()
                        .map(|k| k.frame)
                        .collect::<Vec<_>>(),
                    bone.rotation.keys.iter().map(|k| k.frame).collect(),
                    bone.scale.keys.iter().map(|k| k.frame).collect(),
                ] {
                    assert!(
                        frames.windows(2).all(|w| w[0] < w[1]),
                        "{path} {}",
                        bone.name
                    );
                    assert!(
                        frames.iter().all(|&f| f < anm.frames),
                        "{path} {}",
                        bone.name
                    );
                }
            }
            count += 1;
        }
    }
    assert!(count > 0, "no anm files in {dir}");
}

/// Reads every `.anm` below `AFF_ANM_DIR`, loose files of any version, e.g. a checkout of BI's
/// Arma-Reforger-Samples (SET6): `AFF_ANM_DIR=<dir> cargo test anm_dir_test`.
#[test]
fn anm_dir_test() {
    fn visit(path: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, files);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("anm"))
            {
                files.push(path);
            }
        }
    }

    let Ok(dir) = std::env::var("AFF_ANM_DIR") else {
        return;
    };
    let mut files = Vec::new();
    visit(Path::new(&dir), &mut files);
    assert!(!files.is_empty(), "no anm files in {dir}");
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        let path = path.display();
        let anm = read(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(write(&anm), bytes, "{path}");
        assert!(anm.unknown_chunks.is_empty(), "{path}");
        assert!(anm.bones.iter().all(|b| b.unknown_flags == 0), "{path}");
    }
}

/// `p_t` in the SET6 layout, assembled by hand from the layout of BI's samples.
fn set6_p_t() -> Vec<u8> {
    let mut head = Vec::new();
    for v in [0.0f32, 3.0, 0.0, 1.0, 1.0, 0.0] {
        head.extend(v.to_le_bytes());
    }
    // frames, t, q and s keys, flags (key on every frame of t), name.
    for v in [2u16, 2, 1, 0] {
        head.extend(v.to_le_bytes());
    }
    head.extend([0x10, 1, b'b']);
    let data = probe_bytes("p_t")[78..].to_vec();
    let mut set = Vec::new();
    for (tag, chunk) in [
        (b"FPS\0", 30u32.to_le_bytes().to_vec()),
        (b"HEAD", head),
        (b"DATA", data),
    ] {
        set.extend(tag);
        set.extend((chunk.len() as u32).to_be_bytes());
        set.extend(chunk);
    }
    let mut out = b"FORM".to_vec();
    out.extend((set.len() as u32 + 12).to_be_bytes());
    out.extend(b"ANIMSET6");
    out.extend((set.len() as u32).to_be_bytes());
    out.extend(set);
    out
}

#[test]
fn anm_set6_layout_test() {
    let mut anm = probe("p_t");
    anm.version = AnmVersion::Set6;
    let bytes = set6_p_t();
    assert_eq!(write(&anm), bytes);
    assert_eq!(read(&bytes).unwrap(), anm);

    // SET6 flags tracks without translation (0x02) or rotation (0x04) keys.
    let mut anm = probe("p_qs");
    anm.version = AnmVersion::Set6;
    let bytes = write(&anm);
    // FORM, SET6, FPS and HEAD headers, the ranges and the counts.
    let flags_at = 12 + 8 + 12 + 8 + 24 + 8;
    let flags = bytes[flags_at];
    assert_eq!(flags, 0x02 | 0x20);
    anm.bones[0].rotation.keys.clear();
    let flags = write(&anm)[flags_at];
    assert_eq!(flags, 0x02 | 0x04);
    assert_eq!(read(&write(&anm)).unwrap(), anm);
}

const SET6_PATH_PREFIX: &str = "./tests/enfusion/test-data/anm_in/";

/// Files of BI's Arma-Reforger-Samples (Arma Public License), built by an older Workbench.
fn set6_sample(name: &str) -> (Vec<u8>, Anm) {
    let bytes = std::fs::read(format!("{SET6_PATH_PREFIX}{name}.anm")).unwrap();
    let anm = read(&bytes).unwrap();
    assert_eq!(anm.version, AnmVersion::Set6, "{name}");
    assert_eq!(write(&anm), bytes, "{name}");
    assert!(anm.unknown_chunks.is_empty(), "{name}");
    (bytes, anm)
}

#[test]
fn anm_set6_samples_test() {
    // Values from the sample's `.txa`.
    let (_, brake) = set6_sample("p_SampleCar_01_brake");
    assert_eq!((brake.frames, brake.fps), (2, 30));
    let names: Vec<_> = brake.bones.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "RightLeg",
            "RightLegVolume",
            "RightLegTwist",
            "RightKnee",
            "RightKneeVolume",
            "RightKneeTwist",
            "RightFoot",
            "RightToe"
        ]
    );
    let leg = &brake.bones[0];
    // SET6 truncates, a value is up to a step below.
    assert_values_within(
        &leg.rotation,
        &[
            (0, [0.007_740_84, 0.765_371_8, -0.638_411_9, -0.081_095_12]),
            (1, [0.018_353_57, 0.777_457_95, -0.623_072, -0.083_686_78]),
        ],
        1.0,
    );
    // Quantized by truncation: `(1, 0, 0.145)` of the range became `fffe 0000 2524`.
    assert_eq!(
        leg.translation.keys,
        [AnmKey {
            frame: 0,
            value: [0xfffe, 0, 0x2524]
        }]
    );
    assert_eq!(
        brake.properties,
        Some(vec![(
            "profile".to_string(),
            "Vehicles_Throttle".to_string()
        )])
    );

    let (_, light) = set6_sample("v_SampleCar_01_light_in");
    assert_eq!(light.frames, 35);
    assert_eq!(
        light.events,
        Some(AnmEvents::Plain(vec![AnmPlainEvent {
            frame: 18,
            name: "Sound".into(),
            user_string: "SOUND_VEHICLE_LIGHT_ON".into(),
            user_int: 2,
            frame_count: 0,
            comment: String::new(),
            main_path_only: 1,
        }]))
    );

    let (_, mag) = set6_sample("w_rfl_ak74_erc_insert_mag_tacticool");
    let Some(AnmEvents::FramedObjects { frames, objects }) = &mag.events else {
        panic!("{:?}", mag.events)
    };
    assert_eq!(frames, &[[29, 0], [42, 0], [43, 0], [65, 0]]);
    let first = objects.entries[0].as_object().unwrap();
    assert_eq!(first.key.as_deref(), Some("AnimSrcEventGeneric"));
    assert_eq!(
        first.get("Name"),
        Some(&Value::String("Weapon_SpawnMagazine".into()))
    );
    assert_eq!(first.get("Frame"), None);

    // A range that no step gives back, kept to write it again.
    let (_, insert) = set6_sample("p_rfl_sampleweapon_01_pne_mag_insert");
    let kept: Vec<_> = insert
        .bones
        .iter()
        .flat_map(|b| {
            [
                b.translation.set6_range,
                b.rotation.set6_range,
                b.scale.set6_range,
            ]
        })
        .flatten()
        .collect();
    assert!(!kept.is_empty());
    assert!(kept.iter().all(|&r| r < 1.0));
}
