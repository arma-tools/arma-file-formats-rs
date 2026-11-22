use std::{
    fmt::Write as _,
    io::{Read, Write},
};

use crate::errors::{AffError, LipError};

/// The frame length `WAVToLIP` writes and works with, in seconds.
pub const LIP_FRAME: f64 = 0.04;

/// The highest [`LipKey::value`] `WAVToLIP` writes, the mouth wide open.
pub const LIP_MAX_VALUE: u8 = 7;

/// Upper bounds (inclusive) of the loudness of a frame, relative to the loudest one, for the
/// values `0` to `6`. Louder frames get [`LIP_MAX_VALUE`].
const THRESHOLDS: [f64; 7] = [0.05, 0.2, 0.35, 0.5, 0.65, 0.8, 0.95];

/// The value of the line that ends the lip sync.
const END_VALUE: &str = "-1";

/// A lip sync, see the [module docs](super).
#[derive(Debug, Clone, PartialEq)]
pub struct Lip {
    /// Length of a frame in seconds, [`LIP_FRAME`] in every file.
    pub frame: f64,
    /// The values over time, in the order of the file (ascending time).
    pub keys: Vec<LipKey>,
    /// When the sound ends, in seconds. `None` for silent sounds, which have no keys.
    pub end: Option<f64>,
}

/// How open the mouth is from [`LipKey::time`] on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LipKey {
    /// Seconds since the start of the sound.
    pub time: f64,
    /// `0` (closed) to [`LIP_MAX_VALUE`].
    pub value: u8,
}

impl Default for Lip {
    fn default() -> Self {
        Self {
            frame: LIP_FRAME,
            keys: Vec::new(),
            end: None,
        }
    }
}

impl Lip {
    /// Reads a lip sync. Line ends may be `\r\n` or `\n`, spaces around the numbers and empty
    /// lines are ignored.
    pub fn from<I: Read>(input: &mut I) -> Result<Self, AffError> {
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        Ok(Self::parse(&bytes)?)
    }

    fn parse(bytes: &[u8]) -> Result<Self, LipError> {
        let text = std::str::from_utf8(bytes).map_err(|_| LipError::InvalidText)?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut lines = text
            .split('\n')
            .enumerate()
            .map(|(i, line)| (i + 1, line.trim()))
            .filter(|(_, line)| !line.is_empty());

        let frame = lines
            .next()
            .and_then(|(_, line)| line.strip_prefix("frame"))
            .and_then(|rest| rest.trim_start().strip_prefix('='))
            .and_then(|value| value.trim().parse().ok())
            .ok_or(LipError::MissingFrame)?;

        let mut lip = Self {
            frame,
            ..Self::default()
        };
        for (number, line) in lines {
            if lip.end.is_some() {
                return Err(LipError::AfterEnd(number));
            }
            let invalid = || LipError::InvalidLine(number, line.to_owned());
            let (time, value) = line.split_once(',').ok_or_else(invalid)?;
            let time = time.trim().parse().map_err(|_| invalid())?;
            let value = value.trim();
            if value == END_VALUE {
                lip.end = Some(time);
            } else {
                let value = value.parse().map_err(|_| invalid())?;
                lip.keys.push(LipKey { time, value });
            }
        }
        Ok(lip)
    }

    /// Writes the lip sync like `WAVToLIP`: times with 3 decimals, `\r\n` line ends.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let mut text = format!("frame = {}\r\n", Seconds(self.frame));
        for key in &self.keys {
            let _ = write!(text, "{}, {}\r\n", Seconds(key.time), key.value);
        }
        if let Some(end) = self.end {
            let _ = write!(text, "{}, {END_VALUE}\r\n", Seconds(end));
        }
        out.write_all(text.as_bytes())?;
        Ok(())
    }

    /// Makes the lip sync of 16 bit mono samples like `WAVToLIP` does of a `.wav`, which only
    /// takes 16 bit mono files.
    ///
    /// The samples are cut into frames of [`LIP_FRAME`] seconds (rounded down to whole samples,
    /// a last partial frame is dropped). The loudness of a frame is the sum of how far each
    /// sample is from the first one of the frame. Divided by the loudest frame it picks the
    /// value: up to `0.05` is `0`, up to `0.2` is `1`, and so on in steps of `0.15` up to `0.95`
    /// (`6`), above is [`LIP_MAX_VALUE`]. A sound without frames or where all frames are flat
    /// (silence, but also a constant offset) gets no keys and no end, like `WAVToLIP`'s
    /// "contain silent only".
    #[must_use]
    pub fn from_pcm(samples: &[i16], sample_rate: u32) -> Self {
        // Truncated like the C cast `WAVToLIP` uses.
        let frame_samples = (f64::from(sample_rate) * LIP_FRAME) as usize;
        if frame_samples == 0 {
            return Self::default();
        }
        let loudness: Vec<f64> = samples
            .chunks_exact(frame_samples)
            .map(|frame| {
                let first = i32::from(frame[0]);
                let sum: u64 = frame[1..]
                    .iter()
                    .map(|&s| u64::from((i32::from(s) - first).unsigned_abs()))
                    .sum();
                sum as f64
            })
            .collect();
        let loudest = loudness.iter().copied().fold(0.0, f64::max);
        if loudest <= 0.0 {
            return Self::default();
        }

        let scale = 1.0 / loudest;
        let step = 1.0 / f64::from(sample_rate) * frame_samples as f64;
        let mut lip = Self::default();
        let mut time = 0.0;
        for sum in loudness {
            let relative = sum * scale;
            let value = THRESHOLDS
                .iter()
                .position(|&threshold| relative <= threshold)
                .map_or(LIP_MAX_VALUE, |i| i as u8);
            if lip.keys.last().is_none_or(|key| key.value != value) {
                lip.keys.push(LipKey { time, value });
            }
            // A running sum, not `index * step`, the times are rounded like `WAVToLIP`'s.
            time += step;
        }
        lip.end = Some(time);
        lip
    }

    /// The value at `time` seconds: that of the last key at or before it. `None` before the
    /// first key and from [`Lip::end`] on.
    #[must_use]
    pub fn value_at(&self, time: f64) -> Option<u8> {
        if self.end.is_some_and(|end| time >= end) {
            return None;
        }
        let after = self.keys.partition_point(|key| key.time <= time);
        after.checked_sub(1).map(|i| self.keys[i].value)
    }
}

/// Seconds with 3 decimals like C's `%.3f` of `WAVToLIP`, which rounds a tie away from zero
/// (`0.3125` is `0.313`) where Rust's `{:.3}` rounds it to even.
struct Seconds(f64);

impl std::fmt::Display for Seconds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // A tie is `(2n + 1) / 2000`, as a binary fraction that is an odd number of 16ths.
        let sixteenths = self.0 * 16.0;
        if sixteenths.fract() == 0.0 && sixteenths % 2.0 != 0.0 && sixteenths.abs() < 1e12 {
            let away = self.0 + self.0.signum() / 4000.0;
            return write!(f, "{away:.3}");
        }
        write!(f, "{:.3}", self.0)
    }
}
