use std::{
    io::{Read, Seek, Write},
    time::Duration,
};

use super::delta::{decode_delta4, decode_delta8, encode_delta4, encode_delta8};
use crate::errors::{AffError, WssError};

/// The signature of a `.wss`.
pub const WSS_SIGNATURE: &[u8; 4] = b"WSS0";
/// `WAVE_FORMAT_PCM`, the format tag `WAVToWSS` writes.
pub const WAVE_FORMAT_PCM: u16 = 1;
/// Size of the header before the data.
pub const WSS_HEADER_SIZE: usize = 26;

/// Real Virtuality sound (`.wss`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wss {
    pub compression: WssCompression,
    /// The PCM the data decodes to. The deltapacks decode to 16 bit samples, `WAVToWSS` writes
    /// them with this format.
    pub format: WssFormat,
    /// The `u16` after the format. `WAVToWSS` writes `0`, for the 4 bit deltapack the size of
    /// the decoded PCM in bytes, truncated to 16 bits. `-convert` ignores it. 16 of the game's
    /// 8 bit deltapacks have other values.
    pub extra: u16,
    /// The samples, interleaved by channel: PCM as in the `.wav` for
    /// [`WssCompression::None`], else one byte or nibble per sample.
    pub data: Vec<u8>,
}

/// How [`Wss::data`] stores the samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WssCompression {
    /// `0`: PCM as in [`Wss::format`], what `WAVToWSS` writes without `-d8` or `-d4` (and for
    /// stereo without `-stereo`).
    None,
    /// `8`: the 8 bit deltapack (`-d8`), one byte per sample, see
    /// [`DELTA8_STEPS`](super::DELTA8_STEPS).
    Delta8,
    /// `4`: the 4 bit deltapack (`-d4`, "voice"), two samples per byte, high nibble first,
    /// see [`DELTA4_STEPS`](super::DELTA4_STEPS).
    Delta4,
}

/// A `WAVEFORMAT` with `wBitsPerSample`, the format of a `.wav` file's `fmt ` chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WssFormat {
    /// [`WAVE_FORMAT_PCM`].
    pub format_tag: u16,
    pub channels: u16,
    /// In Hz.
    pub sample_rate: u32,
    /// `sample_rate * block_align`.
    pub byte_rate: u32,
    /// Bytes per sample frame (all channels).
    pub block_align: u16,
    pub bits_per_sample: u16,
}

impl WssCompression {
    /// The `u32` in the header.
    #[must_use]
    pub const fn id(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Delta8 => 8,
            Self::Delta4 => 4,
        }
    }

    #[must_use]
    pub const fn from_id(id: u32) -> Option<Self> {
        match id {
            0 => Some(Self::None),
            8 => Some(Self::Delta8),
            4 => Some(Self::Delta4),
            _ => None,
        }
    }
}

impl WssFormat {
    /// 16 bit PCM with these channels and rate.
    #[must_use]
    pub const fn pcm16(channels: u16, sample_rate: u32) -> Self {
        let block_align = channels * 2;
        Self {
            format_tag: WAVE_FORMAT_PCM,
            channels,
            sample_rate,
            byte_rate: sample_rate * block_align as u32,
            block_align,
            bits_per_sample: 16,
        }
    }

    fn to_bytes(self) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[0..2].copy_from_slice(&self.format_tag.to_le_bytes());
        bytes[2..4].copy_from_slice(&self.channels.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.sample_rate.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.byte_rate.to_le_bytes());
        bytes[12..14].copy_from_slice(&self.block_align.to_le_bytes());
        bytes[14..16].copy_from_slice(&self.bits_per_sample.to_le_bytes());
        bytes
    }

    fn from_bytes(b: &[u8; 16]) -> Self {
        let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        let u32_at = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        Self {
            format_tag: u16_at(0),
            channels: u16_at(2),
            sample_rate: u32_at(4),
            byte_rate: u32_at(8),
            block_align: u16_at(12),
            bits_per_sample: u16_at(14),
        }
    }
}

impl Wss {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        Ok(Self::parse(bytes)?)
    }

    fn parse(mut bytes: Vec<u8>) -> Result<Self, WssError> {
        if bytes.len() < 4 || &bytes[..4] != WSS_SIGNATURE {
            return Err(WssError::UnknownSignature(
                String::from_utf8_lossy(&bytes[..bytes.len().min(4)]).into_owned(),
            ));
        }
        if bytes.len() < WSS_HEADER_SIZE {
            return Err(WssError::Truncated);
        }
        let id = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let compression = WssCompression::from_id(id).ok_or(WssError::UnknownCompression(id))?;
        let format = WssFormat::from_bytes(bytes[8..24].try_into().unwrap_or(&[0; 16]));
        let extra = u16::from_le_bytes([bytes[24], bytes[25]]);
        let data = bytes.split_off(WSS_HEADER_SIZE);
        Ok(Self {
            compression,
            format,
            extra,
            data,
        })
    }

    /// Writes the sound at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        out.write_all(WSS_SIGNATURE)?;
        out.write_all(&self.compression.id().to_le_bytes())?;
        out.write_all(&self.format.to_bytes())?;
        out.write_all(&self.extra.to_le_bytes())?;
        out.write_all(&self.data)?;
        Ok(())
    }

    /// Encodes interleaved 16 bit samples like `WAVToWSS` (with `-stereo` for stereo): PCM for
    /// [`WssCompression::None`], else deltapacked. The 4 bit deltapack drops an odd last
    /// sample.
    #[must_use]
    pub fn from_pcm(
        samples: &[i16],
        channels: u16,
        sample_rate: u32,
        compression: WssCompression,
    ) -> Self {
        let (data, extra) = match compression {
            WssCompression::None => (samples.iter().flat_map(|s| s.to_le_bytes()).collect(), 0),
            WssCompression::Delta8 => (encode_delta8(samples, channels), 0),
            WssCompression::Delta4 => {
                let data = encode_delta4(samples, channels);
                // The size of the PCM, truncated like WAVToWSS does.
                (data, (samples.len() / 2 * 4) as u16)
            }
        };
        Self {
            compression,
            format: WssFormat::pcm16(channels, sample_rate),
            extra,
            data,
        }
    }

    /// Samples (all channels) in [`Wss::data`].
    #[must_use]
    pub fn sample_count(&self) -> usize {
        match self.compression {
            WssCompression::None => self
                .data
                .len()
                .checked_div(usize::from(self.format.bits_per_sample / 8))
                .unwrap_or_default(),
            WssCompression::Delta8 => self.data.len(),
            WssCompression::Delta4 => self.data.len() * 2,
        }
    }

    /// Length of the sound.
    #[must_use]
    pub fn duration(&self) -> Duration {
        let rate = f64::from(self.format.sample_rate) * f64::from(self.format.channels.max(1));
        if rate == 0.0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(self.sample_count() as f64 / rate)
    }

    /// The interleaved 16 bit samples, like `WAVToWSS -convert` decodes them. PCM of other bit
    /// depths is [`WssError::UnsupportedBits`], see [`Wss::pcm`].
    pub fn decode(&self) -> Result<Vec<i16>, WssError> {
        let channels = self.format.channels;
        Ok(match self.compression {
            WssCompression::None if self.format.bits_per_sample == 16 => self
                .data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&b| i16::from_le_bytes(b))
                .collect(),
            WssCompression::None => {
                return Err(WssError::UnsupportedBits(self.format.bits_per_sample));
            }
            WssCompression::Delta8 => decode_delta8(&self.data, channels),
            WssCompression::Delta4 => decode_delta4(&self.data, channels),
        })
    }

    /// The PCM in [`Wss::format`]: [`Wss::data`] for [`WssCompression::None`], else the decoded
    /// samples.
    #[must_use]
    pub fn pcm(&self) -> Vec<u8> {
        match self.compression {
            WssCompression::None => self.data.clone(),
            WssCompression::Delta8 => to_bytes(&decode_delta8(&self.data, self.format.channels)),
            WssCompression::Delta4 => to_bytes(&decode_delta4(&self.data, self.format.channels)),
        }
    }

    /// Writes [`Wss::pcm`] as a `.wav` file, byte for byte like `WAVToWSS -convert`: a RIFF
    /// `WAVE` with an 18 byte `fmt ` chunk ([`Wss::format`] and a `cbSize` of `0`) and the
    /// `data` chunk.
    pub fn write_wav<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let pcm = self.pcm();
        let data_size = u32::try_from(pcm.len()).map_err(|_| WssError::TooLarge)?;
        let riff_size = data_size.checked_add(38).ok_or(WssError::TooLarge)?;
        out.write_all(b"RIFF")?;
        out.write_all(&riff_size.to_le_bytes())?;
        out.write_all(b"WAVEfmt ")?;
        out.write_all(&18u32.to_le_bytes())?;
        out.write_all(&self.format.to_bytes())?;
        out.write_all(&0u16.to_le_bytes())?;
        out.write_all(b"data")?;
        out.write_all(&data_size.to_le_bytes())?;
        // Without the pad byte RIFF wants after odd sizes, like `-convert`.
        out.write_all(&pcm)?;
        Ok(())
    }
}

fn to_bytes(samples: &[i16]) -> Vec<u8> {
    samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}
