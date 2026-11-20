//! Tests for the `.wss` sound reader, writer and deltapack codecs.
//!
//! `test-data/wss_in/` holds four Arma 3 files: `mech_trigger_1.wss` (8 bit deltapack, mono),
//! `grass_9mm_04.wss` (8 bit deltapack, stereo), `hoover1_sheep.wss` (8 bit deltapack with a
//! nonzero `extra`) and `danger.wss` (8 bit PCM), one mod file of the 4 bit deltapack
//! (`voice_d4.wss`), and in `convert/` what `WAVToWSS -convert` of the Arma 3 Tools decodes them
//! to. `wavtowss/` holds generated `.wav` files and what `WAVToWSS` made of them (`*_d8.wss` with
//! `-d8`, `*_d4.wss` with `-d4`, `*_pcm.wss` without options, stereo with `-stereo`).

use std::io::Cursor;

use arma_file_formats::{
    errors::{AffError, WssError},
    real_virtuality::wss::{
        DELTA4_STEP_15, DELTA4_STEPS, DELTA8_STEPS, Wss, WssCompression, WssFormat, decode_delta4,
        decode_delta8, delta8_step, encode_delta4, encode_delta8,
    },
};

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/wss_in/";

const FILES: [&str; 5] = [
    "mech_trigger_1",
    "grass_9mm_04",
    "hoover1_sheep",
    "danger",
    "voice_d4",
];

fn input(name: &str) -> Vec<u8> {
    std::fs::read(format!("{INPUT_PATH_PREFIX}{name}")).unwrap()
}

fn read(bytes: &[u8]) -> Result<Wss, AffError> {
    Wss::from(&mut Cursor::new(bytes))
}

fn write(wss: &Wss) -> Vec<u8> {
    let mut out = Vec::new();
    wss.write(&mut out).unwrap();
    out
}

fn wss_error(result: Result<Wss, AffError>) -> WssError {
    match result {
        Err(AffError::WssError(e)) => e,
        other => panic!("expected a WssError, got {other:?}"),
    }
}

/// Channels and samples of a 16 bit `.wav`.
fn wav_samples(bytes: &[u8]) -> (u16, Vec<i16>) {
    let mut pos = 12;
    let mut channels = 0;
    let mut samples = Vec::new();
    while pos + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = &bytes[pos + 8..pos + 8 + size];
        match &bytes[pos..pos + 4] {
            b"fmt " => channels = u16::from_le_bytes([body[2], body[3]]),
            b"data" => {
                samples = body
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&b| i16::from_le_bytes(b))
                    .collect();
            }
            _ => {}
        }
        pos += 8 + size + (size & 1);
    }
    (channels, samples)
}

#[test]
fn wss_round_trip_test() {
    for name in FILES {
        let bytes = input(&format!("{name}.wss"));
        let wss = read(&bytes).unwrap();
        assert_eq!(write(&wss), bytes, "{name}");
    }
}

#[test]
fn wss_write_wav_like_convert_test() {
    for name in FILES {
        let wss = read(&input(&format!("{name}.wss"))).unwrap();
        let mut wav = Vec::new();
        wss.write_wav(&mut wav).unwrap();
        assert_eq!(wav, input(&format!("convert/{name}.wav")), "{name}");
    }
}

#[test]
fn wss_reencode_game_files_test() {
    // The decoded samples encode back to the game's bytes.
    for name in FILES {
        let wss = read(&input(&format!("{name}.wss"))).unwrap();
        let channels = wss.format.channels;
        match wss.compression {
            WssCompression::Delta8 => {
                assert_eq!(encode_delta8(&wss.decode().unwrap(), channels), wss.data);
            }
            WssCompression::Delta4 => {
                assert_eq!(encode_delta4(&wss.decode().unwrap(), channels), wss.data);
            }
            WssCompression::None => {}
        }
    }
}

#[test]
fn wss_header_test() {
    let wss = read(&input("mech_trigger_1.wss")).unwrap();
    assert_eq!(wss.compression, WssCompression::Delta8);
    assert_eq!(wss.format, WssFormat::pcm16(1, 44100));
    assert_eq!(wss.extra, 0);
    assert_eq!(wss.data.len(), 883);
    assert_eq!(wss.sample_count(), 883);
    assert_eq!(wss.duration().as_micros(), 20_022);

    let wss = read(&input("grass_9mm_04.wss")).unwrap();
    assert_eq!(wss.compression, WssCompression::Delta8);
    assert_eq!(wss.format, WssFormat::pcm16(2, 44100));
    assert_eq!(wss.decode().unwrap().len(), wss.data.len());

    let wss = read(&input("hoover1_sheep.wss")).unwrap();
    assert_eq!(wss.compression, WssCompression::Delta8);
    assert_ne!(wss.extra, 0);

    let wss = read(&input("voice_d4.wss")).unwrap();
    assert_eq!(wss.compression, WssCompression::Delta4);
    assert_eq!(wss.sample_count(), wss.data.len() * 2);
    assert_eq!(wss.extra, (wss.data.len() * 4) as u16);
}

#[test]
fn wss_pcm8_test() {
    let wss = read(&input("danger.wss")).unwrap();
    assert_eq!(wss.compression, WssCompression::None);
    assert_eq!(wss.format.bits_per_sample, 8);
    assert_eq!(wss.format.sample_rate, 8000);
    assert_eq!(wss.sample_count(), wss.data.len());
    assert_eq!(wss.decode(), Err(WssError::UnsupportedBits(8)));
    assert_eq!(wss.pcm(), wss.data);
}

#[test]
fn wss_from_pcm_like_wavtowss_test() {
    for (wav, wss, compression) in [
        ("noise_mono", "noise_mono_d8", WssCompression::Delta8),
        ("noise_mono", "noise_mono_d4", WssCompression::Delta4),
        ("noise_stereo", "noise_stereo_d8", WssCompression::Delta8),
        ("noise_stereo", "noise_stereo_d4", WssCompression::Delta4),
        // 3 samples, the 4 bit deltapack drops the last.
        ("odd_mono", "odd_mono_d4", WssCompression::Delta4),
        ("sine_mono", "sine_mono_pcm", WssCompression::None),
    ] {
        let (channels, samples) = wav_samples(&input(&format!("wavtowss/{wav}.wav")));
        let encoded = Wss::from_pcm(&samples, channels, 44100, compression);
        assert_eq!(
            write(&encoded),
            input(&format!("wavtowss/{wss}.wss")),
            "{wss}"
        );
    }
}

#[test]
fn wss_errors_test() {
    assert_eq!(
        wss_error(read(b"RIFF\0\0\0\0WAVE")),
        WssError::UnknownSignature("RIFF".to_string())
    );
    assert_eq!(wss_error(read(b"WSS0\x08\0\0\0")), WssError::Truncated);

    let mut bytes = input("mech_trigger_1.wss");
    bytes[4] = 2;
    assert_eq!(wss_error(read(&bytes)), WssError::UnknownCompression(2));
}

#[test]
fn wss_delta8_steps_test() {
    assert_eq!(DELTA8_STEPS[0], 0);
    assert_eq!(DELTA8_STEPS[127], 32768);
    assert!(DELTA8_STEPS.windows(2).all(|w| w[0] <= w[1]));
    for v in 1..128u8 {
        assert_eq!(delta8_step(v), -delta8_step(v.wrapping_neg()));
    }
    assert_eq!(delta8_step(0x80), 0);
}

// What `WAVToWSS -convert` decodes crafted data to.

#[test]
fn wss_delta8_decode_test() {
    // Every index followed by its negation.
    let data: Vec<u8> = (0..128u8).flat_map(|v| [v, v.wrapping_neg()]).collect();
    let samples = decode_delta8(&data, 1);
    for v in 0..128 {
        let step = DELTA8_STEPS[v].min(32767) as i16;
        assert_eq!(samples[v * 2..v * 2 + 2], [step, 0]);
    }

    // The sums aren't clamped, only the samples.
    let samples = decode_delta8(&[0x7f, 0x7f, 0x7f, 0x7f, 0x81, 0x81, 0x81, 0x81, 0x81], 1);
    assert_eq!(
        samples,
        [32767, 32767, 32767, 32767, 32767, 32767, 32767, 0, -32768]
    );

    // They wrap in 32 bits.
    let samples = decode_delta8(&[0x7f; 65536], 1);
    assert_eq!(samples[65534], 32767);
    assert_eq!(samples[65535], -32768);

    // Channels have their own sums.
    assert_eq!(
        decode_delta8(&[0x30, 0xd0, 0x30, 0xd0], 2),
        [51, -51, 102, -102]
    );
}

#[test]
fn wss_delta4_decode_test() {
    // High nibble first.
    assert_eq!(decode_delta4(&[0x10], 1), [-4096, -12288]);
    for (n, step) in DELTA4_STEPS.iter().enumerate() {
        assert_eq!(decode_delta4(&[((n as u8) << 4) | 7], 1), [*step as i16; 2]);
    }

    // Nibble 15, which WAVToWSS never writes.
    let mut data = vec![0x7f];
    data.extend([0; 800]);
    let samples = decode_delta4(&data, 1);
    let after_15 = (DELTA4_STEP_15 - 8192 * 1460) as i16;
    assert_eq!(samples[1460], 32767);
    assert_eq!(samples[1461], after_15);
    assert_eq!(after_15, 26268);

    // The sums wrap in 32 bits.
    let samples = decode_delta4(&[0xff; 100], 1);
    assert_eq!(samples[178], 32767);
    assert_eq!(samples[179], -32768);
}

#[test]
fn wss_delta8_encode_test() {
    // Differences the f32 ratio rounds differently than exact math.
    for (d, v) in [
        (3448, 99),
        (3449, 100),
        (20887, 121),
        (20888, 122),
        (22670, 123),
    ] {
        assert_eq!(encode_delta8(&[d], 1), [v], "{d}");
        assert_eq!(encode_delta8(&[-d], 1), [(-(v as i8)) as u8], "{d}");
    }
    assert_eq!(encode_delta8(&[1, 0, 2], 1), [0, 0, 8]);
    assert_eq!(
        encode_delta8(&[i16::MIN, i16::MAX, i16::MIN], 1),
        [0x81, 0x7f, 0x81]
    );
}
