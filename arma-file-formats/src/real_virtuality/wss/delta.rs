//! The deltapack codecs.

/// Steps of the 8 bit deltapack for the indices `0` to `127`, a byte `v` adds `DELTA8_STEPS[v]`
/// for `v < 128` and `-DELTA8_STEPS[256 - v]` above (`0x80` adds 0).
///
/// Step `i` is `r^i` rounded, with `r = 2^(15/127)` as `f32` and the powers multiplied up in
/// `f32`, so `127` is `32768` and the indices `1` to `7` add `1` or `2`.
pub const DELTA8_STEPS: [i32; 128] = [
    0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 10, 11, 12,
    13, 14, 15, 16, 18, 19, 21, 22, 24, 26, 29, 31, 34, 37, 40, 43, 47, 51, 55, 60, 65, 71, 77, 83,
    90, 98, 106, 115, 125, 136, 148, 160, 174, 189, 205, 222, 241, 262, 284, 308, 334, 363, 394,
    428, 464, 504, 547, 593, 644, 699, 758, 823, 893, 970, 1052, 1142, 1240, 1345, 1460, 1585,
    1720, 1867, 2026, 2199, 2386, 2590, 2811, 3050, 3311, 3593, 3900, 4232, 4593, 4985, 5411, 5872,
    6373, 6917, 7507, 8147, 8842, 9597, 10416, 11304, 12268, 13315, 14451, 15684, 17022, 18474,
    20050, 21761, 23617, 25632, 27819, 30192, 32768,
];

/// Steps of the 4 bit deltapack, by nibble. `WAVToWSS` never writes the nibble `15`, its
/// `-convert` adds `11,986,588` for it, see [`DELTA4_STEP_15`].
pub const DELTA4_STEPS: [i32; 15] = [
    -8192, -4096, -2048, -1024, -512, -256, -64, 0, 64, 256, 512, 1024, 2048, 4096, 8192,
];

/// What `WAVToWSS -convert` adds for the 4 bit deltapack nibble `15`, which it never writes.
pub const DELTA4_STEP_15: i32 = 11_986_588;

/// The smallest `|difference|` the 8 bit encoder writes as index `i + 1`: it takes the index
/// `round(ln|d| / ln r)` (at most `127`) for the difference `d` between the sample and the
/// decoded previous one, with the `f32` `r` of [`DELTA8_STEPS`]. Differences of `±1` are `0`.
const DELTA8_THRESHOLDS: [u32; 127] = [
    2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 6, 6, 7, 7, 8, 9, 9, 10, 11, 12,
    13, 14, 15, 16, 17, 19, 20, 22, 24, 26, 28, 30, 33, 36, 39, 42, 46, 49, 54, 58, 63, 68, 74, 80,
    87, 95, 103, 111, 121, 131, 142, 154, 167, 182, 197, 214, 232, 252, 273, 296, 322, 349, 379,
    411, 446, 484, 525, 570, 619, 671, 729, 791, 858, 931, 1011, 1097, 1190, 1292, 1402, 1522,
    1651, 1792, 1945, 2111, 2291, 2486, 2698, 2929, 3178, 3449, 3744, 4063, 4410, 4786, 5194, 5637,
    6118, 6640, 7206, 7821, 8488, 9212, 9998, 10851, 11777, 12782, 13872, 15055, 16340, 17734,
    19247, 20888, 22670, 24605, 26704, 28982, 31454,
];

/// The step a byte of the 8 bit deltapack adds.
#[must_use]
pub const fn delta8_step(byte: u8) -> i32 {
    if byte < 128 {
        DELTA8_STEPS[byte as usize]
    } else if byte == 128 {
        0
    } else {
        -DELTA8_STEPS[256 - byte as usize]
    }
}

/// The step a nibble of the 4 bit deltapack adds.
#[must_use]
pub const fn delta4_step(nibble: u8) -> i32 {
    match nibble & 15 {
        15 => DELTA4_STEP_15,
        n => DELTA4_STEPS[n as usize],
    }
}

/// The byte the 8 bit encoder writes for a difference `d`.
fn delta8_index(d: i32) -> u8 {
    let magnitude = d.unsigned_abs();
    // Indices whose threshold the magnitude reaches.
    let index = DELTA8_THRESHOLDS.partition_point(|&t| t <= magnitude);
    if index == 0 {
        0
    } else {
        let index = index as u8;
        if d < 0 { index.wrapping_neg() } else { index }
    }
}

/// The nibble the 4 bit encoder writes for a difference `d`: the nearest step, on a tie the
/// larger one.
fn delta4_index(d: i32) -> u8 {
    let mut best = 0;
    for (i, step) in DELTA4_STEPS.iter().enumerate() {
        let (distance, best_distance) = ((step - d).abs(), (DELTA4_STEPS[best] - d).abs());
        if distance < best_distance
            || (distance == best_distance && step.abs() > DELTA4_STEPS[best].abs())
        {
            best = i;
        }
    }
    best as u8
}

/// Decodes the 8 bit deltapack: each channel starts at `0`, the steps are added in `i32`
/// (wrapping) and the samples are the sums clamped to `i16`.
#[must_use]
pub fn decode_delta8(data: &[u8], channels: u16) -> Vec<i16> {
    let mut decoder = Decoder::new(channels);
    data.iter().map(|&b| decoder.add(delta8_step(b))).collect()
}

/// Decodes the 4 bit deltapack like [`decode_delta8`], two samples per byte, high nibble first.
#[must_use]
pub fn decode_delta4(data: &[u8], channels: u16) -> Vec<i16> {
    let mut decoder = Decoder::new(channels);
    data.iter()
        .flat_map(|&b| [b >> 4, b & 15])
        .map(|n| decoder.add(delta4_step(n)))
        .collect()
}

/// Encodes interleaved samples as the 8 bit deltapack, like `WAVToWSS -d8`: per sample the index
/// of the step for the difference to what the previous sample of the channel decodes to.
#[must_use]
pub fn encode_delta8(samples: &[i16], channels: u16) -> Vec<u8> {
    let mut decoder = Decoder::new(channels);
    samples
        .iter()
        .map(|&s| {
            let byte = delta8_index(i32::from(s).wrapping_sub(decoder.current()));
            decoder.add(delta8_step(byte));
            byte
        })
        .collect()
}

/// Encodes interleaved samples as the 4 bit deltapack, like `WAVToWSS -d4`: the nearest step
/// per sample as in [`encode_delta8`], two per byte. An odd last sample is dropped.
#[must_use]
pub fn encode_delta4(samples: &[i16], channels: u16) -> Vec<u8> {
    let mut decoder = Decoder::new(channels);
    let mut nibble = |s: i16| {
        let n = delta4_index(i32::from(s).wrapping_sub(decoder.current()));
        decoder.add(delta4_step(n));
        n
    };
    samples
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[first, second]| (nibble(first) << 4) | nibble(second))
        .collect()
}

/// The running sums of the channels.
struct Decoder {
    sums: Vec<i32>,
    channel: usize,
}

impl Decoder {
    fn new(channels: u16) -> Self {
        Self {
            sums: vec![0; usize::from(channels.max(1))],
            channel: 0,
        }
    }

    /// The sum of the current channel.
    fn current(&self) -> i32 {
        self.sums[self.channel]
    }

    /// Adds a step to the current channel, returns its sample and moves to the next channel.
    fn add(&mut self, step: i32) -> i16 {
        let sum = &mut self.sums[self.channel];
        *sum = sum.wrapping_add(step);
        let sample = (*sum).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        self.channel = (self.channel + 1) % self.sums.len();
        sample
    }
}
