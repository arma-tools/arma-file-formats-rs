//! Tests for the `.rtm` animation reader and writer.
//!
//! `test-data/rtm_in/` holds three Arma 3 files: `para_flight.rtm` (`BMTR`, 2 frames of 8
//! bones, arrays stored as is), `amovpercmrunslowwlnrdb.rtm` (`BMTR`, 34 frames of 103 bones,
//! LZO arrays, `StepSound` events) and `catshark_swimcycle_long.rtm` (`RTM_0101`), two mod
//! files of the older versions (`g36.rtm`, `stryker_dead.rtm`), and in `binarize/` sources
//! and what Binarize of the Arma 3 Tools made of them (`*_src.rtm`, `*_out.rtm`).

use std::io::Cursor;

use arma_file_formats::{
    errors::{AffError, RtmError},
    real_virtuality::rtm::{
        AnimMetadata, Bmtr, BmtrFrame, BmtrKeystone, BmtrTransform, KEYSTONE_STEP_SOUND,
        KEYSTONE_UNKNOWN, RTM_IDENTITY, Rtm, RtmFile, RtmFrame, RtmKeystone, RtmMetadata,
        RtmMetadataValue, RtmName, RtmTransform, RtmVersion,
    },
};
use half::f16;

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/rtm_in/";

fn game_file(name: &str) -> Vec<u8> {
    std::fs::read(format!("{INPUT_PATH_PREFIX}{name}")).unwrap()
}

fn read(bytes: &[u8]) -> Result<RtmFile, AffError> {
    RtmFile::from(&mut Cursor::new(bytes))
}

fn write(file: &RtmFile) -> Vec<u8> {
    let mut out = Vec::new();
    file.write(&mut out).unwrap();
    out
}

fn rtm_error(result: Result<impl std::fmt::Debug, AffError>) -> RtmError {
    match result {
        Err(AffError::RtmError(e)) => e,
        other => panic!("expected an RtmError, got {other:?}"),
    }
}

fn binarized(bytes: &[u8]) -> Bmtr {
    match read(bytes).unwrap() {
        RtmFile::Binarized(rtm) => rtm,
        other => panic!("expected BMTR, got {other:?}"),
    }
}

fn plain(bytes: &[u8]) -> Rtm {
    match read(bytes).unwrap() {
        RtmFile::Plain(rtm) => rtm,
        other => panic!("expected RTM_0101, got {other:?}"),
    }
}

fn norm(q: [f32; 4]) -> f32 {
    q.iter().map(|c| c * c).sum::<f32>().sqrt()
}

#[test]
fn bmtr_game_static() {
    let rtm = binarized(&game_file("para_flight.rtm"));
    assert_eq!(rtm.version, 5);
    assert!(rtm.flipped_xz);
    assert_eq!(rtm.step, [0.0; 3]);
    assert!(rtm.preload);
    assert_eq!(
        rtm.bones,
        ["body", "striska", "fl", "bl", "fm", "mb", "fr", "rb"]
    );
    assert!(rtm.metadata.is_empty());
    assert!(rtm.keystones.is_empty());
    assert_eq!(rtm.frames.len(), 2);
    assert_eq!(rtm.frames[0].time, 0.0);
    assert_eq!(rtm.frames[1].time, 0.1);
    for frame in &rtm.frames {
        assert_eq!(frame.transforms, vec![BmtrTransform::IDENTITY; 8]);
    }
    assert_eq!(rtm.bone_index("STRISKA"), Some(1));
    assert_eq!(rtm.bone_index("nope"), None);
}

#[test]
fn bmtr_game_moving() {
    let rtm = binarized(&game_file("amovpercmrunslowwlnrdb.rtm"));
    assert_eq!(rtm.step, [0.0, 0.0, 4.207_155]);
    assert!(!rtm.preload);
    assert_eq!(rtm.bones.len(), 103);
    assert_eq!(rtm.bones[..3], ["weapon", "launcher", "rightarm"]);
    assert_eq!(rtm.bones[102], "eyeright");
    let events: Vec<_> = rtm
        .keystones
        .iter()
        .map(|k| (k.kind, k.name.as_str(), k.time, k.value.as_str()))
        .collect();
    assert_eq!(
        events,
        [
            (0, "StepSound", 0.212, ""),
            (0, "StepSound", 0.424, ""),
            (0, "StepSound", 0.666, ""),
            (0, "StepSound", 0.909, ""),
        ]
    );
    assert_eq!(rtm.frames.len(), 34);
    assert_eq!(rtm.frames[0].time, 0.0);
    assert_eq!(rtm.frames[1].time, 1.0 / 33.0);
    assert_eq!(rtm.frames[33].time, 1.0);

    let weapon = rtm.frames[0].transforms[0];
    assert_eq!(weapon.rotation, [6751, 10182, 9112, 6013]);
    assert_eq!(
        weapon.translation(),
        [0.541_503_9, 0.262_939_45, -1.106_445_3]
    );
    for frame in &rtm.frames {
        assert_eq!(frame.transforms.len(), 103);
        for transform in &frame.transforms {
            assert!((norm(transform.rotation()) - 1.0).abs() < 2e-3);
        }
    }
}

/// The written file is not the game's (other LZO output) but reads back to the same animation
/// and writes the same bytes again.
#[test]
fn bmtr_game_round_trip() {
    // Arrays stored as is, and LZO compressed like Binarize.
    for name in ["para_flight.rtm", "amovpercmrunslowwlnrdb.rtm"] {
        let bytes = game_file(name);
        assert_eq!(write(&read(&bytes).unwrap()), bytes, "{name}");
    }
}

#[test]
fn rtm_game() {
    let bytes = game_file("catshark_swimcycle_long.rtm");
    let rtm = plain(&bytes);
    assert_eq!(rtm.step[1..], [0.0, -2.0]);
    let names: Vec<_> = rtm.bones.iter().map(|b| b.to_string_lossy()).collect();
    assert_eq!(
        names,
        ["fish_head", "fish_tail1", "fish_tail2", "fish_body"]
    );
    assert_eq!(rtm.frames.len(), 21);
    assert_eq!(rtm.frames[0].time, 0.0);
    assert_eq!(rtm.frames[20].time, 1.0);
    for frame in &rtm.frames {
        for (bone, transform) in rtm.bones.iter().zip(&frame.transforms) {
            assert_eq!(bone.as_bytes(), transform.bone.as_bytes());
        }
    }
    // The bytes after the NUL are leftovers of the exporter, different in the frames.
    assert_ne!(rtm.bones[0], rtm.frames[0].transforms[0].bone);
    assert_eq!(rtm.bone_index("FISH_BODY"), Some(3));

    assert_eq!(write(&RtmFile::Plain(rtm.clone())), bytes);
    let mut out = Vec::new();
    rtm.write(&mut out).unwrap();
    assert_eq!(out, bytes);
    assert_eq!(Rtm::from(&mut Cursor::new(&bytes)).unwrap(), rtm);
}

fn sample_rtm() -> Rtm {
    let bone = RtmName::new("head").unwrap();
    let mut moved = RTM_IDENTITY;
    moved[3] = [0.0, 0.5, 0.25];
    Rtm {
        step: [0.0, 0.0, 1.5],
        version: RtmVersion::Rtm0101,
        metadata: None,
        bones: vec![bone],
        frames: vec![
            RtmFrame {
                time: 0.0,
                transforms: vec![RtmTransform {
                    bone,
                    matrix: RTM_IDENTITY,
                }],
            },
            RtmFrame {
                time: 1.0,
                transforms: vec![RtmTransform {
                    bone,
                    matrix: moved,
                }],
            },
        ],
    }
}

fn floats(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn name32(name: &str) -> Vec<u8> {
    let mut bytes = name.as_bytes().to_vec();
    bytes.resize(32, 0);
    bytes
}

#[test]
fn rtm_write_layout() {
    let mut expected = b"RTM_0101".to_vec();
    expected.extend(floats(&[0.0, 0.0, 1.5]));
    expected.extend(2u32.to_le_bytes());
    expected.extend(1u32.to_le_bytes());
    expected.extend(name32("head"));
    for (time, position) in [(0.0, [0.0, 0.0, 0.0]), (1.0, [0.0, 0.5, 0.25])] {
        expected.extend(floats(&[time]));
        expected.extend(name32("head"));
        expected.extend(floats(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
        expected.extend(floats(&position));
    }
    let rtm = sample_rtm();
    let mut out = Vec::new();
    rtm.write(&mut out).unwrap();
    assert_eq!(out, expected);
    assert_eq!(plain(&expected), rtm);
}

#[test]
fn rtm_names() {
    let name = RtmName::new("leftHand").unwrap();
    assert_eq!(name.as_bytes(), b"leftHand");
    assert!(name.matches("LEFTHAND"));
    assert!(!name.matches("lefthand2"));
    assert_eq!(RtmName::new(&"x".repeat(31)).unwrap().as_bytes().len(), 31);
    assert_eq!(
        RtmName::new(&"x".repeat(32)),
        Err(RtmError::InvalidBoneName("x".repeat(32)))
    );
    assert!(RtmName::new("a\0b").is_err());
    // No NUL in all 32 bytes: the whole buffer is the name.
    assert_eq!(RtmName([b'a'; 32]).as_bytes(), [b'a'; 32]);
    assert_eq!(RtmName::default().as_bytes(), b"");
}

#[test]
fn rtm_errors() {
    let mut bytes = Vec::new();
    sample_rtm().write(&mut bytes).unwrap();
    for len in [0, 3, 7, 8, 27, 60, bytes.len() - 1] {
        assert_eq!(rtm_error(read(&bytes[..len])), RtmError::Truncated, "{len}");
    }
    assert_eq!(
        rtm_error(Rtm::from(&mut Cursor::new(b"BMTR\x05\0\0\0"))),
        RtmError::UnknownSignature("BMTR....".into())
    );
    assert_eq!(rtm_error(read(b"RTM_MDAT")), RtmError::Truncated);
    assert_eq!(
        rtm_error(read(b"RTM_MDAT\0\0\0\0\0\0\0\0RTM_0102")),
        RtmError::UnknownSignature("RTM_0102".into())
    );
    assert_eq!(
        rtm_error(read(b"RTM_MDAT\0\0\0\0\0\0\0\0RTM_MDAT")),
        RtmError::UnknownSignature("RTM_MDAT".into())
    );

    // A huge frame count fails on the missing data, without allocating for it.
    let mut huge = bytes.clone();
    huge[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(rtm_error(read(&huge)), RtmError::Truncated);

    let mut rtm = sample_rtm();
    rtm.frames[1].transforms.clear();
    assert_eq!(
        rtm_error(rtm.write(&mut Vec::new())),
        RtmError::TransformCountMismatch {
            frame: 1,
            expected: 1,
            actual: 0
        }
    );
}

fn sample_bmtr() -> Bmtr {
    let half_turn = std::f32::consts::FRAC_1_SQRT_2;
    let moved = BmtrTransform::new([0.0, 0.0, half_turn, half_turn], [0.0, 0.25, -1.5]);
    Bmtr {
        version: 5,
        flipped_xz: true,
        step: [0.0, 0.0, 2.0],
        preload: false,
        bones: vec!["weapon".into(), "head".into()],
        metadata: Vec::new(),
        keystones: vec![BmtrKeystone {
            kind: KEYSTONE_STEP_SOUND,
            name: "StepSound".into(),
            time: 0.5,
            value: "left".into(),
        }],
        frames: vec![
            BmtrFrame {
                time: 0.0,
                transforms: vec![BmtrTransform::IDENTITY; 2],
            },
            BmtrFrame {
                time: 1.0,
                transforms: vec![BmtrTransform::IDENTITY, moved],
            },
        ],
    }
}

/// [`sample_bmtr`] with both arrays stored as is.
fn sample_bmtr_bytes() -> Vec<u8> {
    let mut out = b"BMTR".to_vec();
    out.extend(5u32.to_le_bytes());
    out.push(1);
    out.extend(floats(&[0.0, 0.0, 2.0]));
    out.extend(2u32.to_le_bytes()); // frames
    out.extend(0u32.to_le_bytes()); // preload
    out.extend(2u32.to_le_bytes()); // bones
    out.extend(2u32.to_le_bytes()); // names
    out.extend(b"weapon\0head\0");
    out.extend(0u32.to_le_bytes());
    out.extend(1u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(b"StepSound\0");
    out.extend(floats(&[0.5]));
    out.extend(b"left\0");
    out.extend(2u32.to_le_bytes());
    out.push(0);
    out.extend(floats(&[0.0, 1.0]));
    let identity = [0, 0, 0, 0, 0, 0, 0, 0x40, 0, 0, 0, 0, 0, 0];
    for second in [identity.to_vec(), {
        let mut t = Vec::new();
        for c in [0i16, 0, 11585, 11585] {
            t.extend(c.to_le_bytes());
        }
        for c in [0.0f32, 0.25, -1.5] {
            t.extend(f16::from_f32(c).to_le_bytes());
        }
        t
    }] {
        out.extend(2u32.to_le_bytes());
        out.push(0);
        out.extend(identity);
        out.extend(second);
    }
    out
}

#[test]
fn bmtr_layout() {
    let bytes = sample_bmtr_bytes();
    let rtm = binarized(&bytes);
    assert_eq!(rtm, sample_bmtr());
    assert_eq!(Bmtr::from(&mut Cursor::new(&bytes)).unwrap(), rtm);

    // Arrays this small are stored as is.
    let mut out = Vec::new();
    rtm.write(&mut out).unwrap();
    assert_eq!(out, bytes);
}

/// `frames` frames of `bones` identity transforms, no metadata.
fn identity_bmtr(version: u32, frames: usize, bones: usize) -> Bmtr {
    Bmtr {
        version,
        flipped_xz: true,
        preload: true,
        bones: (0..bones).map(|i| format!("b{i:02}")).collect(),
        frames: (0..frames)
            .map(|_| BmtrFrame {
                time: 0.0,
                transforms: vec![BmtrTransform::IDENTITY; bones],
            })
            .collect(),
        ..Bmtr::default()
    }
}

/// Offset of the frame times of [`identity_bmtr`].
fn identity_times_at(version: u32, bones: usize) -> usize {
    37 + bones * 4 + if version >= 4 { 8 } else { 0 }
}

#[test]
fn bmtr_lzo_arrays() {
    // Version 5: up to 256 bytes as is, larger arrays LZO compressed (flag 2) if that saves
    // enough. 64 zero frame times (256 bytes) and 18 identity transforms (252 bytes) stay.
    let rtm = identity_bmtr(5, 64, 18);
    let mut written = Vec::new();
    rtm.write(&mut written).unwrap();
    let times_at = identity_times_at(5, 18);
    assert_eq!(written[times_at..times_at + 5], [64, 0, 0, 0, 0]);
    let last_frame = &written[written.len() - (5 + 252)..];
    assert_eq!(last_frame[..5], [18, 0, 0, 0, 0]);
    assert_eq!(binarized(&written), rtm);

    // 65 times (260 bytes) and 19 transforms (266 bytes) compress.
    let rtm = identity_bmtr(5, 65, 19);
    let mut written = Vec::new();
    rtm.write(&mut written).unwrap();
    let times_at = identity_times_at(5, 19);
    assert_eq!(written[times_at..times_at + 5], [65, 0, 0, 0, 2]);
    assert!(written.len() < times_at + 65 * (5 + 266));
    assert_eq!(binarized(&written), rtm);

    // Before version 5 no flag: LZO from 1024 bytes on, whether that is smaller or not.
    for version in [3, 4] {
        for (frames, bones, compressed) in [(255, 73, false), (256, 74, true)] {
            let rtm = identity_bmtr(version, frames, bones);
            let mut written = Vec::new();
            rtm.write(&mut written).unwrap();
            let times_at = identity_times_at(version, bones);
            assert_eq!(
                written[times_at..times_at + 4],
                (frames as u32).to_le_bytes()
            );
            let raw_len = times_at + 4 + frames * 4 + frames * (4 + bones * 14);
            if compressed {
                assert!(written.len() < raw_len, "v{version}");
            } else {
                assert_eq!(written.len(), raw_len, "v{version}");
            }
            assert_eq!(binarized(&written), rtm);
        }
    }
}

#[test]
fn bmtr_old_versions() {
    // Version 3 has no metadata and keystones.
    let rtm = identity_bmtr(3, 2, 1);
    let mut written = Vec::new();
    rtm.write(&mut written).unwrap();
    let mut expected = b"BMTR".to_vec();
    expected.extend(3u32.to_le_bytes());
    expected.push(1);
    expected.extend([0; 12]);
    for value in [2u32, 1, 1, 1] {
        expected.extend(value.to_le_bytes());
    }
    expected.extend(b"b00\0");
    expected.extend(2u32.to_le_bytes());
    expected.extend([0; 8]);
    for _ in 0..2 {
        expected.extend(1u32.to_le_bytes());
        expected.extend([0, 0, 0, 0, 0, 0, 0, 0x40, 0, 0, 0, 0, 0, 0]);
    }
    assert_eq!(written, expected);
    assert_eq!(binarized(&expected), rtm);

    // Version 4 has them, but no compression flags.
    let mut rtm = identity_bmtr(4, 2, 1);
    rtm.keystones.push(BmtrKeystone {
        name: "StepSound".into(),
        time: 0.25,
        ..BmtrKeystone::default()
    });
    let mut written = Vec::new();
    rtm.write(&mut written).unwrap();
    let mut expected4 = expected.clone();
    expected4[4] = 4;
    let mut keystones = 0u32.to_le_bytes().to_vec();
    keystones.extend(1u32.to_le_bytes());
    keystones.extend(0u32.to_le_bytes());
    keystones.extend(b"StepSound\0");
    keystones.extend(floats(&[0.25]));
    keystones.push(0);
    expected4.splice(41..41, keystones);
    assert_eq!(written, expected4);
    assert_eq!(binarized(&expected4), rtm);

    rtm.version = 3;
    assert_eq!(
        rtm_error(rtm.write(&mut Vec::new())),
        RtmError::MetadataUnsupported
    );
    let mut rtm = identity_bmtr(3, 1, 1);
    rtm.metadata = vec!["1".into()];
    assert_eq!(
        rtm_error(rtm.write(&mut Vec::new())),
        RtmError::MetadataUnsupported
    );
    for version in [2, 6] {
        rtm.version = version;
        assert_eq!(
            rtm_error(rtm.write(&mut Vec::new())),
            RtmError::UnsupportedVersion(version)
        );
    }
}

/// Mod files of the older versions: FFAA's `g36.rtm` (version 4, arrays below 1024 bytes,
/// byte exact) and rhsgref's `stryker_dead.rtm` (version 3, LZO arrays).
#[test]
fn bmtr_game_old_versions() {
    let bytes = game_file("g36.rtm");
    let rtm = binarized(&bytes);
    assert_eq!(rtm.version, 4);
    assert!(rtm.preload);
    assert_eq!(rtm.bones.len(), 67);
    assert_eq!(rtm.bones[0], "rightarm");
    assert!(rtm.keystones.is_empty());
    assert_eq!(rtm.frames.len(), 2);
    assert_eq!(write(&RtmFile::Binarized(rtm)), bytes);

    let bytes = game_file("stryker_dead.rtm");
    let file = read(&bytes).unwrap();
    let RtmFile::Binarized(rtm) = &file else {
        panic!("expected BMTR");
    };
    assert_eq!(rtm.version, 3);
    assert_eq!(rtm.bones.len(), 128);
    assert_eq!(rtm.frames.len(), 2);
    for frame in &rtm.frames {
        for transform in &frame.transforms {
            assert!((norm(transform.rotation()) - 1.0).abs() < 2e-3);
        }
    }
    // The game's LZO stream is a few bytes shorter.
    let written = write(&file);
    assert_ne!(written, bytes);
    assert_eq!(read(&written).unwrap(), file);
}

#[test]
fn bmtr_transforms() {
    let t = BmtrTransform::new([0.5, -0.5, 0.5, -0.5], [1.0, -2.0, 0.333]);
    assert_eq!(t.rotation, [8192, -8192, 8192, -8192]);
    assert_eq!(t.rotation(), [0.5, -0.5, 0.5, -0.5]);
    // Truncated towards zero, the nearest half float would be 0.333_007_8.
    assert_eq!(t.translation(), [1.0, -2.0, 0.332_763_67]);
    // Below the smallest normal half float +0, past the largest exponent sign and 0x7fff.
    let t = BmtrTransform::new([0.0; 4], [3e-5, -1e-7, 65504.0]);
    assert_eq!(t.translation.map(f16::to_bits), [0, 0, 0x7bff]);
    let t = BmtrTransform::new([0.0; 4], [140_000.0, -140_000.0, f32::NAN]);
    assert_eq!(t.translation.map(f16::to_bits), [0x7fff, 0xffff, 0x7fff]);
    // Rounded to the nearest step, saturated at the i16 range.
    assert_eq!(
        BmtrTransform::new([1.0 / 32768.0, -3.0, 2.0, 1.999], [0.0; 3]).rotation,
        [1, -32768, 32767, 32752]
    );
    assert_eq!(BmtrTransform::default().rotation, [0; 4]);
    assert_eq!(BmtrTransform::IDENTITY.rotation(), [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn bmtr_errors() {
    let bytes = sample_bmtr_bytes();
    for len in [0, 4, 8, 20, 40, 60, bytes.len() - 1] {
        assert_eq!(rtm_error(read(&bytes[..len])), RtmError::Truncated, "{len}");
    }
    assert_eq!(
        rtm_error(Bmtr::from(&mut Cursor::new(b"RTM_0101"))),
        RtmError::UnknownSignature("RTM_".into())
    );

    let mut version = bytes.clone();
    version[4] = 6;
    assert_eq!(rtm_error(read(&version)), RtmError::UnsupportedVersion(6));
    version[4] = 2;
    assert_eq!(rtm_error(read(&version)), RtmError::UnsupportedVersion(2));

    let mut names = bytes.clone();
    names[33] = 3;
    assert_eq!(
        rtm_error(read(&names)),
        RtmError::BoneCountMismatch {
            header: 2,
            names: 3
        }
    );

    let times_at = bytes.len() - 2 * (4 + 1 + 28) - (4 + 1 + 8);
    let mut times = bytes.clone();
    times[times_at] = 3;
    assert_eq!(
        rtm_error(read(&times)),
        RtmError::FrameCountMismatch {
            header: 2,
            times: 3
        }
    );

    let mut flag = bytes.clone();
    flag[times_at + 4] = 1;
    assert_eq!(rtm_error(read(&flag)), RtmError::UnknownCompression(1));

    let mut lzo = bytes.clone();
    lzo[times_at + 4] = 2;
    assert!(matches!(rtm_error(read(&lzo)), RtmError::Lzo(_)));

    let second_frame = bytes.len() - (4 + 1 + 28);
    let mut transforms = bytes.clone();
    transforms[second_frame] = 1;
    transforms.truncate(transforms.len() - 14);
    assert_eq!(
        rtm_error(read(&transforms)),
        RtmError::TransformCountMismatch {
            frame: 1,
            expected: 2,
            actual: 1
        }
    );

    let mut text = bytes.clone();
    text[37] = 0xff; // 'w' of weapon
    assert!(matches!(rtm_error(read(&text)), RtmError::InvalidString(_)));

    let mut rtm = sample_bmtr();
    rtm.keystones[0].value = "a\0b".into();
    assert!(matches!(
        rtm_error(rtm.write(&mut Vec::new())),
        RtmError::InvalidString(_)
    ));
    let mut rtm = sample_bmtr();
    rtm.frames[0].transforms.pop();
    assert_eq!(
        rtm_error(rtm.write(&mut Vec::new())),
        RtmError::TransformCountMismatch {
            frame: 0,
            expected: 2,
            actual: 1
        }
    );
}

fn binarize_pair(name: &str) -> (Rtm, Bmtr) {
    let src = plain(&game_file(&format!("binarize/{name}_src.rtm")));
    let out = binarized(&game_file(&format!("binarize/{name}_out.rtm")));
    (src, out)
}

/// Sources and what Binarize (Arma 3 Tools) made of them, with a skeleton holding all their
/// bones: the game's catshark, metadata and keystones (`m_dup`: duplicate keys and equal
/// times, `m_case`: names in other case, `k41`: more keystones than the short sort handles),
/// random rotations with uniform scales (`r_scale`), an `RTM_0100` (`old5`) and an animation
/// without frames.
#[test]
fn bmtr_from_rtm_like_binarize() {
    for name in [
        "catshark_swimcycle_long",
        "m_dup",
        "m_case",
        "k41",
        "r_scale",
        "old5",
        "st_0frames",
        "big_times",
        "long_repeat",
    ] {
        let (src, out) = binarize_pair(name);
        assert_eq!(Bmtr::from_rtm(&src, None), out, "{name}");
    }

    let (_, out) = binarize_pair("catshark_swimcycle_long");
    assert!(out.flipped_xz);
    assert!(!out.preload);
    let (src, out) = binarize_pair("old5");
    assert_eq!(src.version, RtmVersion::Rtm0100);
    assert_eq!(src.step, [0.0, 0.0, 1.25]);
    assert!(out.preload);
}

#[test]
fn bmtr_metadata_and_keystones() {
    let (src, out) = binarize_pair("m_dup");
    let metadata = src.metadata.as_ref().unwrap();
    assert_eq!(
        metadata.values,
        [
            RtmMetadataValue {
                key: "WalkCycles".into(),
                value: "1".into()
            },
            RtmMetadataValue {
                key: "WalkCycles".into(),
                value: "2".into()
            },
        ]
    );
    // The last value of a key wins.
    assert_eq!(out.metadata, ["2"]);
    assert_eq!(out.metadata(AnimMetadata::WalkCycles), Some("2"));
    assert_eq!(out.metadata(AnimMetadata::AnimLength), None);
    // Sorted by time, equal times in the order of Binarize's unstable sort.
    let keystones: Vec<_> = out
        .keystones
        .iter()
        .map(|k| (k.kind, k.name.as_str(), k.time, k.value.as_str()))
        .collect();
    assert_eq!(
        keystones,
        [
            (KEYSTONE_UNKNOWN, "Other", 0.2, "c"),
            (KEYSTONE_STEP_SOUND, "StepSound", 0.5, "b"),
            (KEYSTONE_STEP_SOUND, "StepSound", 0.5, "c"),
            (KEYSTONE_STEP_SOUND, "StepSound", 0.5, "a"),
        ]
    );

    // Keys and keystone names ignore case.
    let (_, out) = binarize_pair("m_case");
    assert_eq!(out.metadata(AnimMetadata::WalkCycles), Some("2"));
    assert_eq!(out.metadata(AnimMetadata::AnimLength), Some("3"));
    assert!(out.keystones.iter().all(|k| k.kind == KEYSTONE_STEP_SOUND));
    assert_eq!(
        AnimMetadata::from_name("animlength"),
        Some(AnimMetadata::AnimLength)
    );
    assert_eq!(AnimMetadata::from_name("Foo"), None);

    // Back to RTM_MDAT: known keys, keystones as they are.
    let rtm = out.to_rtm().unwrap();
    let metadata = rtm.metadata.unwrap();
    let keys: Vec<_> = metadata
        .values
        .iter()
        .map(|v| (v.key.as_str(), v.value.as_str()))
        .collect();
    assert_eq!(keys, [("WalkCycles", "2"), ("AnimLength", "3")]);
    assert_eq!(metadata.keystones.len(), 2);
}

/// `RTM_MDAT` with a `u8` length per string, then the animation.
#[test]
fn rtm_mdat_layout() {
    let short = |s: &str| [&[s.len() as u8], s.as_bytes()].concat();
    let mut bytes = b"RTM_MDAT".to_vec();
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(short("AnimLength"));
    bytes.extend(short("1.5"));
    bytes.extend(2u32.to_le_bytes());
    for (time, value) in [(0.75f32, ""), (0.25, "x")] {
        bytes.extend(time.to_le_bytes());
        bytes.extend(short("StepSound"));
        bytes.extend(short(value));
    }
    let mut body = Vec::new();
    sample_rtm().write(&mut body).unwrap();
    bytes.extend(&body);

    let rtm = plain(&bytes);
    assert_eq!(
        rtm.metadata,
        Some(RtmMetadata {
            values: vec![RtmMetadataValue {
                key: "AnimLength".into(),
                value: "1.5".into()
            }],
            keystones: vec![
                RtmKeystone {
                    time: 0.75,
                    name: "StepSound".into(),
                    value: String::new()
                },
                RtmKeystone {
                    time: 0.25,
                    name: "StepSound".into(),
                    value: "x".into()
                },
            ],
        })
    );
    assert_eq!(
        Rtm {
            metadata: None,
            ..rtm.clone()
        },
        sample_rtm()
    );
    assert_eq!(write(&RtmFile::Plain(rtm.clone())), bytes);

    let bmtr = Bmtr::from_rtm(&rtm, None);
    assert_eq!(bmtr.metadata, ["", "1.5"]);
    assert_eq!(bmtr.keystones[0].time, 0.25);

    let mut long = rtm;
    long.metadata.as_mut().unwrap().keystones[0].value = "x".repeat(256);
    assert_eq!(
        rtm_error(long.write(&mut Vec::new())),
        RtmError::StringTooLong("x".repeat(256))
    );
}

/// `RTM_0100` stores only the z of the step.
#[test]
fn rtm_0100_layout() {
    let mut body = Vec::new();
    sample_rtm().write(&mut body).unwrap();
    let mut bytes = b"RTM_0100".to_vec();
    bytes.extend(1.5f32.to_le_bytes());
    bytes.extend(&body[20..]);
    let rtm = plain(&bytes);
    assert_eq!(rtm.version, RtmVersion::Rtm0100);
    assert_eq!(
        Rtm {
            version: RtmVersion::Rtm0101,
            ..rtm.clone()
        },
        sample_rtm()
    );
    assert_eq!(write(&RtmFile::Plain(rtm.clone())), bytes);

    let mut moved = rtm;
    moved.step[0] = 1.0;
    assert_eq!(
        rtm_error(moved.write(&mut Vec::new())),
        RtmError::StepUnsupported
    );
}

/// Without frames Binarize writes 0 bones in the header, the names still follow.
#[test]
fn bmtr_without_frames() {
    let bytes = game_file("binarize/st_0frames_out.rtm");
    let rtm = binarized(&bytes);
    assert_eq!(rtm.bones, ["b"]);
    assert!(rtm.frames.is_empty());
    assert!(rtm.preload);
    assert_eq!(bytes[29..33], [0, 0, 0, 0]);
    assert_eq!(write(&RtmFile::Binarized(rtm)), bytes);
}

#[test]
fn bmtr_transform_matrices() {
    let rx = [
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, -1.0, 0.0],
        [1.0, 2.0, 3.0],
    ];
    let ry = [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0; 3]];
    let rz = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]];
    let scaled = [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0], [0.0; 3]];
    // What Binarize made of these.
    let t = BmtrTransform::from_matrix(&rx, true);
    assert_eq!(t.rotation, [11585, 0, 0, 11585]);
    assert_eq!(t.translation(), [-1.0, 2.0, -3.0]);
    assert_eq!(
        BmtrTransform::from_matrix(&ry, true).rotation,
        [0, -11585, 0, 11585]
    );
    assert_eq!(
        BmtrTransform::from_matrix(&rz, true).rotation,
        [0, 0, 11585, 11585]
    );
    assert_eq!(
        BmtrTransform::from_matrix(&scaled, true).rotation,
        [0, 0, 0, 23170]
    );
    assert_eq!(
        BmtrTransform::from_matrix(&RTM_IDENTITY, true),
        BmtrTransform::IDENTITY
    );
    // Without the flip the rotation turns the other way and the translation stays.
    let t = BmtrTransform::from_matrix(&rx, false);
    assert_eq!(t.rotation, [-11585, 0, 0, 11585]);
    assert_eq!(t.translation(), [1.0, 2.0, 3.0]);

    for (matrix, flip) in [(rx, true), (ry, true), (rz, false), (scaled, true)] {
        let back = BmtrTransform::from_matrix(&matrix, flip).to_matrix(flip);
        for (row, expected) in back.iter().zip(&matrix) {
            for (v, e) in row.iter().zip(expected) {
                assert!((v - e).abs() < 1e-3, "{back:?} {matrix:?}");
            }
        }
    }
}

#[test]
fn bmtr_to_rtm() {
    let rtm = binarized(&game_file("amovpercmrunslowwlnrdb.rtm"));
    let plain = rtm.to_rtm().unwrap();
    assert_eq!(plain.version, RtmVersion::Rtm0101);
    assert_eq!(plain.bones.len(), 103);
    assert!(plain.bones[0].matches("weapon"));
    assert_eq!(plain.metadata.as_ref().unwrap().keystones.len(), 4);
    assert_eq!(plain.frames.len(), 34);

    // Binarized again: the same up to a step in a few rotations.
    let again = Bmtr::from_rtm(&plain, None);
    assert_eq!(again.bones, rtm.bones);
    assert_eq!(again.keystones, rtm.keystones);
    for (a, b) in again.frames.iter().zip(&rtm.frames) {
        for (x, y) in a.transforms.iter().zip(&b.transforms) {
            assert_eq!(x.translation, y.translation);
            for (p, q) in x.rotation.iter().zip(y.rotation) {
                assert!((p - q).abs() <= 1);
            }
        }
    }

    let mut long = rtm;
    long.bones[0] = "x".repeat(32);
    assert_eq!(
        long.to_rtm(),
        Err(RtmError::InvalidBoneName("x".repeat(32)))
    );
}

/// With a skeleton, bones not in it are dropped, names are lower case.
#[test]
fn bmtr_from_rtm_skeleton() {
    let names = ["B0", "zz", "b1"];
    let bones: Vec<RtmName> = names.iter().map(|n| RtmName::new(n).unwrap()).collect();
    let mut matrices = [RTM_IDENTITY; 3];
    for (i, m) in matrices.iter_mut().enumerate() {
        m[3][1] = i as f32;
    }
    let rtm = Rtm {
        bones: bones.clone(),
        frames: vec![RtmFrame {
            time: 0.0,
            transforms: bones
                .iter()
                .zip(matrices)
                .map(|(&bone, matrix)| RtmTransform { bone, matrix })
                .collect(),
        }],
        ..Rtm::default()
    };
    let out = Bmtr::from_rtm(&rtm, Some(&["b0", "B1", "b2"]));
    assert_eq!(out.bones, ["b0", "b1"]);
    let ys: Vec<f32> = out.frames[0]
        .transforms
        .iter()
        .map(|t| t.translation()[1])
        .collect();
    assert_eq!(ys, [0.0, 2.0]);
    assert_eq!(Bmtr::from_rtm(&rtm, None).bones, ["b0", "zz", "b1"]);
}

/// Binarizing and writing gives Binarize's files, LZO arrays included: `big_times` has a
/// 10,800 byte array (compressed in blocks), `long_repeat` matches over 1024 bytes.
#[test]
fn bmtr_written_like_binarize() {
    for name in [
        "catshark_swimcycle_long",
        "m_dup",
        "m_case",
        "k41",
        "r_scale",
        "old5",
        "st_0frames",
        "big_times",
        "long_repeat",
    ] {
        let src = plain(&game_file(&format!("binarize/{name}_src.rtm")));
        let out = game_file(&format!("binarize/{name}_out.rtm"));
        let mut written = Vec::new();
        Bmtr::from_rtm(&src, None).write(&mut written).unwrap();
        assert_eq!(written, out, "{name}");
    }
}
