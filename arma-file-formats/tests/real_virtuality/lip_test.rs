//! Tests for the `.lip` lip sync reader, writer and `Lip::from_pcm`.
//!
//! `test-data/lip_in/` holds Arma 3 files: `CloseThatDoor.lip` (`dubbing_radio_f_data_engb.pbo`),
//! `artillery_accomplished.lip` (`dubbing_f.pbo`), the silent `a_m03_50_reinf_dead_MIL_0.lip`
//! (`dubbing_f_epa.pbo`) and the one frame `veh_vehicle_armedcar_s.lip`
//! (`dubbing_radio_f_data_eng.pbo`). `lip_in/wavtolip/` holds what `WAVToLIP` of the Arma 3 Tools
//! made of the signals of [`signals`], written to 16 bit mono `.wav` files.

use std::io::Cursor;

use arma_file_formats::{
    errors::{AffError, LipError},
    real_virtuality::lip::{LIP_FRAME, LIP_MAX_VALUE, Lip, LipKey},
};

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/lip_in/";

const GAME_FILES: [&str; 4] = [
    "CloseThatDoor.lip",
    "artillery_accomplished.lip",
    "a_m03_50_reinf_dead_MIL_0.lip",
    "veh_vehicle_armedcar_s.lip",
];

fn input(name: &str) -> Vec<u8> {
    std::fs::read(format!("{INPUT_PATH_PREFIX}{name}")).unwrap()
}

fn read(bytes: &[u8]) -> Result<Lip, AffError> {
    Lip::from(&mut Cursor::new(bytes))
}

fn write(lip: &Lip) -> String {
    let mut out = Vec::new();
    lip.write(&mut out).unwrap();
    String::from_utf8(out).unwrap()
}

fn error(bytes: &[u8]) -> LipError {
    match read(bytes) {
        Err(AffError::LipError(e)) => e,
        other => panic!("expected a LipError, got {other:?}"),
    }
}

const fn key(time: f64, value: u8) -> LipKey {
    LipKey { time, value }
}

/// Xorshift noise, `-32768..32768`.
fn noise(len: usize, seed: u32) -> impl Iterator<Item = i32> {
    let mut state = seed;
    (0..len).map(move |_| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state >> 16) as i32 - 32768
    })
}

/// Noise in "syllables" of 0.2 s, each rising and falling, with 4 loudness levels.
fn speech(len: usize, seed: u32, rate: usize) -> Vec<i16> {
    noise(len, seed)
        .enumerate()
        .map(|(i, n)| {
            let syllable = (i * 5 / rate) % 4;
            let phase = (i * 5 % rate) as i32;
            let half = rate as i32 / 2;
            let envelope = [0, 3, 8, 5][syllable] * (half - (phase - half).abs()) / half;
            (n * envelope / 8) as i16
        })
        .collect()
}

/// A triangle wave getting louder.
fn triangle(len: usize, period: usize) -> Vec<i16> {
    (0..len)
        .map(|i| {
            let p = (i % period) as i32;
            let half = period as i32 / 2;
            let amp = (i * 30000 / len) as i32;
            (((p - half).abs() * 2 - half) * amp / half) as i16
        })
        .collect()
}

/// Frames whose only non zero sample is the second, `sums` are their loudness. With the loudest
/// at 1280 they hit each threshold exactly and once just above.
fn steps(sums: &[i16], frame: usize) -> Vec<i16> {
    sums.iter()
        .flat_map(|&sum| {
            let mut samples = vec![0; frame];
            samples[1] = sum;
            samples
        })
        .collect()
}

const STEPS: [i16; 17] = [
    1280, 64, 65, 256, 257, 448, 449, 640, 641, 832, 833, 1024, 1025, 1216, 1217, 1280, 0,
];

/// Name of the `WAVToLIP` output, samples, sample rate.
fn signals() -> Vec<(&'static str, Vec<i16>, u32)> {
    vec![
        ("steps_44100", steps(&STEPS, 1764), 44100),
        ("speech_44100", speech(88200, 1, 44100), 44100),
        ("triangle_22050", triangle(33075, 50), 22050),
        // 10 samples per frame of 0.0390625 s, the times hit ties of `%.3f`.
        ("noise_256", speech(768, 7, 256), 256),
        // 493 samples per frame, not quite 0.04 s.
        ("speech_12345", speech(30000, 3, 12345), 12345),
        // "contain silent only", only the frame line.
        ("offset_8000", vec![1000; 8000], 8000),
    ]
}

#[test]
fn game_files_round_trip_byte_exact() {
    for name in GAME_FILES {
        let bytes = input(name);
        let lip = read(&bytes).unwrap();
        assert_eq!(write(&lip).as_bytes(), bytes, "{name}");
    }
}

#[test]
fn reads_game_file() {
    let lip = read(&input("CloseThatDoor.lip")).unwrap();
    assert_eq!(lip.frame, LIP_FRAME);
    assert_eq!(lip.keys.len(), 15);
    assert_eq!(
        &lip.keys[..4],
        &[key(0.0, 1), key(0.04, 3), key(0.08, 4), key(0.16, 7)]
    );
    assert_eq!(lip.keys[14], key(0.8, 0));
    assert_eq!(lip.end, Some(0.96));
    assert!(lip.keys.iter().all(|k| k.value <= LIP_MAX_VALUE));
}

#[test]
fn reads_silent_and_one_frame_files() {
    let silent = read(&input("a_m03_50_reinf_dead_MIL_0.lip")).unwrap();
    assert_eq!(silent, Lip::default());
    assert_eq!(write(&silent), "frame = 0.040\r\n");

    let one = read(&input("veh_vehicle_armedcar_s.lip")).unwrap();
    assert_eq!(one.keys, [key(0.0, 7)]);
    assert_eq!(one.end, Some(0.04));
}

#[test]
fn from_pcm_matches_wavtolip() {
    for (name, samples, rate) in signals() {
        let expected = input(&format!("wavtolip/{name}.lip"));
        let lip = Lip::from_pcm(&samples, rate);
        assert_eq!(
            write(&lip),
            String::from_utf8(expected.clone()).unwrap(),
            "{name}"
        );
        assert_eq!(
            read(&expected).unwrap().keys.len(),
            lip.keys.len(),
            "{name}"
        );
    }
}

#[test]
fn from_pcm_thresholds_are_inclusive() {
    let lip = Lip::from_pcm(&steps(&STEPS, 1764), 44100);
    let values: Vec<u8> = (0..STEPS.len())
        .map(|frame| lip.value_at(frame as f64 * 0.04 + 0.001).unwrap())
        .collect();
    // 0.05, 0.2, 0.5, 0.65 and 0.8 are hit exactly and stay below. 448 and 1216 times
    // `1.0 / 1280.0` round to just above 0.35 and 0.95 (`WAVToLIP` agrees, see `steps_44100`).
    assert_eq!(values, [7, 0, 1, 1, 2, 3, 3, 3, 4, 4, 5, 5, 6, 7, 7, 7, 0]);
}

#[test]
fn from_pcm_without_frames_or_loudness_is_silent() {
    let noise: Vec<i16> = noise(10_000, 5).map(|n| n as i16).collect();
    // No samples, no rate, below one sample per frame, one sample per frame (never loud),
    // shorter than a frame.
    for (samples, rate) in [
        (&[][..], 44100),
        (&noise[..], 0),
        (&noise[..], 24),
        (&noise[..], 49),
        (&noise[..1763], 44100),
    ] {
        assert_eq!(Lip::from_pcm(samples, rate), Lip::default(), "{rate}");
    }
    // The partial last frame is dropped.
    let lip = Lip::from_pcm(&noise[..1764 * 2 + 1000], 44100);
    assert_eq!(lip.end, Some(0.08));
}

#[test]
fn value_at() {
    let lip = Lip {
        keys: vec![key(0.1, 2), key(0.2, 5)],
        end: Some(0.3),
        ..Lip::default()
    };
    assert_eq!(lip.value_at(0.0), None);
    assert_eq!(lip.value_at(0.1), Some(2));
    assert_eq!(lip.value_at(0.19), Some(2));
    assert_eq!(lip.value_at(0.25), Some(5));
    assert_eq!(lip.value_at(0.3), None);
    let open = Lip {
        keys: vec![key(0.0, 1)],
        ..Lip::default()
    };
    assert_eq!(open.value_at(100.0), Some(1));
}

#[test]
fn writes_ties_away_from_zero() {
    let lip = Lip {
        frame: 0.0625,
        keys: vec![
            key(0.039_062_5, 1),
            key(0.3125, 2),
            key(0.9375, 3),
            key(2.8125, 4),
            key(-0.0625, 5),
            key(1e12, 6),
        ],
        end: Some(1.5625),
    };
    assert_eq!(
        write(&lip),
        "frame = 0.063\r\n0.039, 1\r\n0.313, 2\r\n0.938, 3\r\n2.813, 4\r\n-0.063, 5\r\n\
         1000000000000.000, 6\r\n1.563, -1\r\n"
    );
}

#[test]
fn reads_loose_text() {
    let lip = read(b"\xef\xbb\xbf\n  frame=0.04 \n").unwrap();
    assert_eq!(lip, Lip::default());
    let lip = read(b"frame = 0.040\n\n0.000 ,3\n 0.1,  7 \r\n0.2,-1\n\n").unwrap();
    assert_eq!(lip.keys, [key(0.0, 3), key(0.1, 7)]);
    assert_eq!(lip.end, Some(0.2));
}

#[test]
fn read_errors() {
    assert_eq!(error(b"frame = 0.040\r\n\xff"), LipError::InvalidText);
    assert_eq!(error(b""), LipError::MissingFrame);
    assert_eq!(error(b"0.000, 1\r\n"), LipError::MissingFrame);
    assert_eq!(error(b"frame 0.04\r\n"), LipError::MissingFrame);
    assert_eq!(error(b"frame = x\r\n"), LipError::MissingFrame);
    assert_eq!(
        error(b"frame = 0.040\r\n0.000 1\r\n"),
        LipError::InvalidLine(2, "0.000 1".into())
    );
    assert_eq!(
        error(b"frame = 0.040\r\nx, 1\r\n"),
        LipError::InvalidLine(2, "x, 1".into())
    );
    assert_eq!(
        error(b"frame = 0.040\r\n0.000, 256\r\n"),
        LipError::InvalidLine(2, "0.000, 256".into())
    );
    assert_eq!(
        error(b"frame = 0.040\r\n0.000, 1\r\n0.040, -1\r\n\r\n0.080, 2\r\n"),
        LipError::AfterEnd(5)
    );
    // The scripts some mods hide behind the extension.
    assert_eq!(
        error(b"call compileFinal (toArray \"...\")"),
        LipError::MissingFrame
    );
}
