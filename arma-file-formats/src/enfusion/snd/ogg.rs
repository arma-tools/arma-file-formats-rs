//! Ogg Opus output (RFC 7845) for [`Snd`].

use std::io::Write;

use super::Snd;
use crate::errors::{AffError, SndError};

/// Pages are flushed once their packets reach this size, like libogg does.
const PAGE_BODY_TARGET: usize = 4096;
const MAX_LACING_VALUES: usize = 255;

const FLAG_CONTINUED: u8 = 0x01;
const FLAG_FIRST: u8 = 0x02;
const FLAG_LAST: u8 = 0x04;

const VENDOR: &str = concat!("arma-file-formats ", env!("CARGO_PKG_VERSION"));

impl Snd {
    /// Writes the packets as an Ogg Opus file with the stream serial number `serial`.
    ///
    /// The `OpusHead` has [`Snd::pre_skip`], and the last page's granule position ends the
    /// sound after [`Snd::sample_count`] samples, so decoders drop the encoder's delay and the
    /// flush packet like the engine does. The packets are copied, not re-encoded.
    pub fn write_ogg<W: Write>(&self, out: &mut W, serial: u32) -> Result<(), AffError> {
        let channels = self.format.channels;
        // Channel mapping family 0 only covers mono and stereo.
        if !(1..=2).contains(&channels) {
            return Err(SndError::UnsupportedChannels(channels).into());
        }
        let pre_skip =
            u16::try_from(self.pre_skip).map_err(|_| SndError::PreSkipTooLarge(self.pre_skip))?;

        let mut head = b"OpusHead".to_vec();
        head.push(1);
        head.push(channels as u8);
        head.extend(pre_skip.to_le_bytes());
        head.extend(self.format.sample_rate.to_le_bytes());
        // Output gain and channel mapping family.
        head.extend(0i16.to_le_bytes());
        head.push(0);

        let mut tags = b"OpusTags".to_vec();
        tags.extend((VENDOR.len() as u32).to_le_bytes());
        tags.extend(VENDOR.as_bytes());
        // No user comments.
        tags.extend(0u32.to_le_bytes());

        let mut pages = OggWriter {
            out,
            serial,
            sequence: 0,
        };
        pages.page(FLAG_FIRST, 0, &[head.len() as u8], &head)?;
        let lacing = lacing_values(tags.len());
        pages.page(0, 0, &lacing, &tags)?;

        let end = u64::from(self.pre_skip) + u64::from(self.sample_count());
        let mut granule = u64::from(self.pre_skip);
        let mut lacing = Vec::new();
        let mut body = Vec::new();
        // Granule position of the last packet that ends on the current page.
        let mut page_granule = None;
        let mut flags = 0;
        for (i, packet) in self.packets.iter().enumerate() {
            granule += u64::from(packet_samples(packet).ok_or(SndError::InvalidPacket(i))?);
            let last = i + 1 == self.packets.len();

            let mut rest = packet.as_slice();
            loop {
                let len = rest.len().min(255);
                lacing.push(len as u8);
                body.extend(&rest[..len]);
                rest = &rest[len..];
                // A lacing value below 255 ends the packet.
                let packet_done = len < 255;
                if packet_done {
                    page_granule = Some(if last { granule.min(end) } else { granule });
                }
                if lacing.len() == MAX_LACING_VALUES && !(packet_done && last) {
                    // -1: no packet ends on the page.
                    pages.page(flags, page_granule.unwrap_or(u64::MAX), &lacing, &body)?;
                    flags = if packet_done { 0 } else { FLAG_CONTINUED };
                    lacing.clear();
                    body.clear();
                    page_granule = None;
                }
                if packet_done {
                    break;
                }
            }
            if last || body.len() >= PAGE_BODY_TARGET {
                let page_flags = flags | if last { FLAG_LAST } else { 0 };
                pages.page(page_flags, page_granule.unwrap_or(u64::MAX), &lacing, &body)?;
                flags = 0;
                lacing.clear();
                body.clear();
                page_granule = None;
            }
        }
        if self.packets.is_empty() {
            pages.page(FLAG_LAST, granule, &[], &[])?;
        }
        Ok(())
    }
}

/// Samples (per channel, at 48 kHz) of an Opus packet, from its TOC byte (RFC 6716 3.1).
#[must_use]
pub fn packet_samples(packet: &[u8]) -> Option<u32> {
    let toc = *packet.first()?;
    let config = usize::from(toc >> 3);
    let frame_samples = match config {
        // SILK: 10, 20, 40, 60 ms.
        0..=11 => [480, 960, 1920, 2880][config & 3],
        // Hybrid: 10, 20 ms.
        12..=15 => [480, 960][config & 1],
        // CELT: 2.5, 5, 10, 20 ms.
        _ => [120, 240, 480, 960][config & 3],
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => u32::from(*packet.get(1)? & 0x3f),
    };
    Some(frame_samples * frames)
}

fn lacing_values(len: usize) -> Vec<u8> {
    let mut lacing = vec![255; len / 255];
    lacing.push((len % 255) as u8);
    lacing
}

struct OggWriter<'a, W: Write> {
    out: &'a mut W,
    serial: u32,
    sequence: u32,
}

impl<W: Write> OggWriter<'_, W> {
    fn page(&mut self, flags: u8, granule: u64, lacing: &[u8], body: &[u8]) -> std::io::Result<()> {
        let mut page = Vec::with_capacity(27 + lacing.len() + body.len());
        page.extend(b"OggS");
        page.push(0);
        page.push(flags);
        page.extend(granule.to_le_bytes());
        page.extend(self.serial.to_le_bytes());
        page.extend(self.sequence.to_le_bytes());
        // The CRC, filled in below.
        page.extend([0; 4]);
        page.push(lacing.len() as u8);
        page.extend(lacing);
        page.extend(body);
        let crc = crc32(&page);
        page[22..26].copy_from_slice(&crc.to_le_bytes());
        self.sequence += 1;
        self.out.write_all(&page)
    }
}

/// CRC-32 with polynomial `0x04C11DB7`, not reflected, initial value and final XOR `0`.
fn crc32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0, |crc, &b| {
        (crc << 8) ^ CRC_TABLE[usize::from((crc >> 24) as u8 ^ b)]
    })
}

const CRC_TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut r = (i as u32) << 24;
        let mut bit = 0;
        while bit < 8 {
            r = if r & 0x8000_0000 != 0 {
                (r << 1) ^ 0x04C1_1DB7
            } else {
                r << 1
            };
            bit += 1;
        }
        table[i] = r;
        i += 1;
    }
    table
};
