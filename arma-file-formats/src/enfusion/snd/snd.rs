use std::{
    io::{Read, Seek, Write},
    time::Duration,
};

use crate::errors::{AffError, SndError};

const RIFF: &[u8; 4] = b"RIFF";
const SND_FORM_TYPE: &[u8; 4] = b"OPUS";
const VERS: &[u8; 4] = b"VERS";
const FMT: &[u8; 4] = b"fmt ";
const ENV: &[u8; 4] = b"ENV ";
const OPUS: &[u8; 4] = b"opus";
const DATA: &[u8; 4] = b"data";

/// The version Workbench writes.
pub const SND_VERSION: u8 = 5;
/// `WAVE_FORMAT_PCM`, the only format Workbench converts and the format the packets decode to.
pub const WAVE_FORMAT_PCM: u16 = 1;
/// Number of parts of [`SndEnvelope::levels`].
pub const SND_ENVELOPE_LEVELS: usize = 32;

/// Enfusion Opus sound (`.snd`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snd {
    pub version: SndVersion,
    pub format: SndFormat,
    /// The `ENV ` chunk.
    pub envelope: Option<SndEnvelope>,
    /// Number of packets that hold the sound, [`Snd::packets`] has one more (Workbench's flush
    /// packet).
    pub packet_count: u32,
    /// Samples (per channel) to discard at the start of the decoded packets: the libopus
    /// encoder's lookahead, `312` at 48 kHz.
    pub pre_skip: u32,
    /// The size of the `data` chunk: bytes of PCM in [`Snd::format`] that the sound decodes to,
    /// without [`Snd::pre_skip`]. Workbench writes the PCM size of [`Snd::packet_count`]
    /// packets, so the sound ends in up to 40 ms of the padding silence.
    pub data_size: u32,
    /// The Opus packets, without their `u16` length.
    pub packets: Vec<Vec<u8>>,
}

/// The `VERS` chunk. It has no proper size: the size field is the version or `1` before a
/// version byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SndVersion {
    /// A size of `1`, the version byte and 3 zero bytes. Workbench writes [`SND_VERSION`].
    Byte(u8),
    /// The version in the size field, without anything after it. The engine reads `2` to `4`
    /// this way and expects a version byte after other values. One game file has `1`.
    Inline(u32),
}

/// The `fmt ` chunk, a `WAVEFORMAT` with `wBitsPerSample`: the PCM the packets decode to.
/// Every game file is 16 bit stereo at 48 kHz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SndFormat {
    /// [`WAVE_FORMAT_PCM`].
    pub format_tag: u16,
    pub channels: u16,
    /// In Hz, `48000`.
    pub sample_rate: u32,
    /// `sample_rate * block_align`.
    pub byte_rate: u32,
    /// Bytes per sample frame (all channels).
    pub block_align: u16,
    pub bits_per_sample: u16,
}

/// The `ENV ` chunk: the loudness of the sound over time, for the engine's dynamic volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SndEnvelope {
    /// The RMS of 32 equal parts of the source PCM, see [`SndEnvelope::from_pcm`].
    pub levels: [u16; SND_ENVELOPE_LEVELS],
    /// The "Dynamic Volume" option of the conversion (off by default), a byte and 3 zero
    /// bytes. `None` for a chunk of only the levels, which the engine reads for version 2 and
    /// treats as on.
    pub dynamic_volume: Option<bool>,
}

impl SndFormat {
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
}

impl SndEnvelope {
    /// Levels of interleaved 16 bit PCM the way Workbench computes them: the file is split into
    /// 32 parts of `frames / 32` sample frames (dropping the rest), and each level is the root
    /// of the mean over the part's frames of the frame's mean square over its channels, all
    /// truncated.
    ///
    /// Not compared with Workbench output for known PCM: the levels of the decoded game files
    /// match their envelopes within the codec's loss (about 1 %).
    #[must_use]
    pub fn from_pcm(samples: &[i16], channels: u16, dynamic_volume: bool) -> Self {
        let channels = usize::from(channels.max(1));
        let frames = samples.len() / channels;
        let part = frames / SND_ENVELOPE_LEVELS;
        let mut levels = [0; SND_ENVELOPE_LEVELS];
        for (i, level) in levels.iter_mut().enumerate() {
            let range = i * part * channels..(i + 1) * part * channels;
            let sum: u64 = samples[range]
                .chunks_exact(channels)
                .map(|frame| {
                    frame
                        .iter()
                        .map(|&s| i64::from(s).pow(2) as u64)
                        .sum::<u64>()
                        / channels as u64
                })
                .sum();
            // `0 / 0` for fewer than 32 frames, Workbench's NaN converts to 0 as well.
            *level = (sum as f32 / part as f32).sqrt() as u16;
        }
        Self {
            levels,
            dynamic_volume: Some(dynamic_volume),
        }
    }
}

impl Snd {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        Ok(Self::parse(&bytes)?)
    }

    fn parse(bytes: &[u8]) -> Result<Self, SndError> {
        let mut input = Reader { bytes, pos: 0 };
        if input.take(4)? != RIFF {
            return Err(SndError::NotRiff);
        }
        // Header size plus PCM size, not needed.
        input.u32()?;
        let form_type = input.take(4)?;
        if form_type != SND_FORM_TYPE {
            return Err(SndError::UnknownFormType(lossy(form_type)));
        }

        if input.take(4)? != VERS {
            return Err(SndError::ChunkMissing("VERS"));
        }
        let size = input.u32()?;
        let version = if size == 1 && !is_chunk_tag(input.peek(4)?) {
            let version = input.take(4)?[0];
            SndVersion::Byte(version)
        } else {
            SndVersion::Inline(size)
        };

        let mut format = None;
        let mut envelope = None;
        let mut opus = None;
        let data_size = loop {
            let tag: [u8; 4] = input.take(4)?.try_into().unwrap_or_default();
            let size = input.u32()?;
            if &tag == DATA {
                break size;
            }
            let data = input.take(size as usize)?;
            let mut chunk = Reader {
                bytes: data,
                pos: 0,
            };
            let duplicate = match &tag {
                FMT => format.is_some().then_some("fmt "),
                ENV => envelope.is_some().then_some("ENV "),
                OPUS => opus.is_some().then_some("opus"),
                _ => None,
            };
            if let Some(tag) = duplicate {
                return Err(SndError::DuplicateChunk(tag));
            }
            match &tag {
                FMT if size == 16 => {
                    format = Some(SndFormat {
                        format_tag: chunk.u16()?,
                        channels: chunk.u16()?,
                        sample_rate: chunk.u32()?,
                        byte_rate: chunk.u32()?,
                        block_align: chunk.u16()?,
                        bits_per_sample: chunk.u16()?,
                    });
                }
                ENV if size == 64 || size == 68 => {
                    let mut levels = [0; SND_ENVELOPE_LEVELS];
                    for level in &mut levels {
                        *level = chunk.u16()?;
                    }
                    envelope = Some(SndEnvelope {
                        levels,
                        dynamic_volume: (size == 68).then(|| data[64] != 0),
                    });
                }
                OPUS if size == 8 => opus = Some((chunk.u32()?, chunk.u32()?)),
                FMT => return Err(SndError::ChunkSize { tag: "fmt ", size }),
                ENV => return Err(SndError::ChunkSize { tag: "ENV ", size }),
                OPUS => return Err(SndError::ChunkSize { tag: "opus", size }),
                _ => return Err(SndError::UnknownChunk(lossy(&tag))),
            }
        };
        let format = format.ok_or(SndError::ChunkMissing("fmt "))?;
        let (packet_count, pre_skip) = opus.ok_or(SndError::ChunkMissing("opus"))?;

        // The count is untrusted, don't allocate more than the data can hold.
        let mut packets = Vec::with_capacity((packet_count as usize + 1).min(bytes.len() / 2));
        while input.pos < bytes.len() {
            let len = input.u16()?;
            packets.push(input.take(usize::from(len))?.to_vec());
        }

        Ok(Self {
            version,
            format,
            envelope,
            packet_count,
            pre_skip,
            data_size,
            packets,
        })
    }

    /// Writes the sound at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let mut header = Vec::with_capacity(0x94);
        header.extend(RIFF);
        // The RIFF size, filled in below.
        header.extend([0; 4]);
        header.extend(SND_FORM_TYPE);

        header.extend(VERS);
        match self.version {
            SndVersion::Byte(version) => {
                header.extend(1u32.to_le_bytes());
                header.extend([version, 0, 0, 0]);
            }
            SndVersion::Inline(version) => header.extend(version.to_le_bytes()),
        }

        let f = &self.format;
        header.extend(FMT);
        header.extend(16u32.to_le_bytes());
        header.extend(f.format_tag.to_le_bytes());
        header.extend(f.channels.to_le_bytes());
        header.extend(f.sample_rate.to_le_bytes());
        header.extend(f.byte_rate.to_le_bytes());
        header.extend(f.block_align.to_le_bytes());
        header.extend(f.bits_per_sample.to_le_bytes());

        if let Some(envelope) = &self.envelope {
            header.extend(ENV);
            let size: u32 = if envelope.dynamic_volume.is_some() {
                68
            } else {
                64
            };
            header.extend(size.to_le_bytes());
            for level in envelope.levels {
                header.extend(level.to_le_bytes());
            }
            if let Some(dynamic_volume) = envelope.dynamic_volume {
                header.extend([u8::from(dynamic_volume), 0, 0, 0]);
            }
        }

        header.extend(OPUS);
        header.extend(8u32.to_le_bytes());
        header.extend(self.packet_count.to_le_bytes());
        header.extend(self.pre_skip.to_le_bytes());

        header.extend(DATA);
        header.extend(self.data_size.to_le_bytes());

        let riff_size = u32::try_from(header.len())
            .ok()
            .and_then(|len| len.checked_add(self.data_size))
            .ok_or(SndError::TooLarge)?;
        header[4..8].copy_from_slice(&riff_size.to_le_bytes());
        out.write_all(&header)?;

        for (i, packet) in self.packets.iter().enumerate() {
            let len = u16::try_from(packet.len()).map_err(|_| SndError::PacketTooLarge(i))?;
            out.write_all(&len.to_le_bytes())?;
            out.write_all(packet)?;
        }
        Ok(())
    }

    /// Sample frames (samples per channel) of the sound, from [`Snd::data_size`].
    #[must_use]
    pub fn sample_count(&self) -> u32 {
        self.data_size
            .checked_div(u32::from(self.format.block_align))
            .unwrap_or_default()
    }

    /// Length of the sound, from [`Snd::data_size`].
    #[must_use]
    pub fn duration(&self) -> Duration {
        if self.format.sample_rate == 0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(f64::from(self.sample_count()) / f64::from(self.format.sample_rate))
    }
}

fn is_chunk_tag(bytes: &[u8]) -> bool {
    [FMT, ENV, OPUS, DATA].iter().any(|tag| bytes == *tag)
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The data is in memory, so reads only fail at its end.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn peek(&self, len: usize) -> Result<&'a [u8], SndError> {
        self.pos
            .checked_add(len)
            .and_then(|end| self.bytes.get(self.pos..end))
            .ok_or(SndError::Truncated)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], SndError> {
        let bytes = self.peek(len)?;
        self.pos += len;
        Ok(bytes)
    }

    fn u16(&mut self) -> Result<u16, SndError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().unwrap_or_default(),
        ))
    }

    fn u32(&mut self) -> Result<u32, SndError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }
}
