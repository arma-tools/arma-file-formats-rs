//! Tests for the snd reader, writer and Ogg Opus output.
//!
//! The hand built sounds follow Workbench's writer (the layout of every game file but one) and
//! the header of `Sounds/Music/KS/Samples/MU_MU_Spectre_Basso.snd`, the one without version byte
//! and `ENV `. `snd_game_*` read two of the game's 154 sounds from `test-data/snd_in/`, one with
//! "Dynamic Volume" on.

use std::io::Cursor;

use arma_file_formats::{
    enfusion::snd::{
        SND_VERSION, Snd, SndEnvelope, SndFormat, SndVersion, WAVE_FORMAT_PCM, packet_samples,
    },
    errors::{AffError, SndError},
};

fn read(bytes: &[u8]) -> Result<Snd, AffError> {
    Snd::from(&mut Cursor::new(bytes))
}

fn write(snd: &Snd) -> Vec<u8> {
    let mut out = Vec::new();
    snd.write(&mut out).unwrap();
    out
}

fn snd_error(result: Result<impl std::fmt::Debug, AffError>) -> SndError {
    match result {
        Err(AffError::SndError(e)) => e,
        other => panic!("expected an SndError, got {other:?}"),
    }
}

/// CELT fullband 20 ms stereo, 2 frames: 40 ms like Workbench's packets.
fn packet(len: usize) -> Vec<u8> {
    let mut packet = vec![0xAB; len];
    packet[0] = 0xFE;
    packet
}

/// A sound like Workbench writes it: `VERS` 5, `ENV ` with the option byte, 2 + 1 packets.
fn workbench_snd() -> Snd {
    let mut levels = [0; 32];
    for (i, level) in levels.iter_mut().enumerate() {
        *level = 1000 + i as u16;
    }
    Snd {
        version: SndVersion::Byte(SND_VERSION),
        format: SndFormat::pcm16(2, 48000),
        envelope: Some(SndEnvelope {
            levels,
            dynamic_volume: Some(true),
        }),
        packet_count: 2,
        pre_skip: 312,
        data_size: 2 * 1920 * 4,
        packets: vec![packet(300), packet(310), vec![0xFD, 0xFF, 0xFE, 0xFF, 0xFE]],
    }
}

fn workbench_bytes() -> Vec<u8> {
    let mut out = b"RIFF".to_vec();
    out.extend((0x94u32 + 2 * 1920 * 4).to_le_bytes());
    out.extend(b"OPUS");
    out.extend(b"VERS");
    out.extend([1, 0, 0, 0, 5, 0, 0, 0]);
    out.extend(b"fmt ");
    out.extend(16u32.to_le_bytes());
    out.extend([1, 0, 2, 0]);
    out.extend(48000u32.to_le_bytes());
    out.extend(192_000u32.to_le_bytes());
    out.extend([4, 0, 16, 0]);
    out.extend(b"ENV ");
    out.extend(68u32.to_le_bytes());
    for i in 0..32u16 {
        out.extend((1000 + i).to_le_bytes());
    }
    out.extend([1, 0, 0, 0]);
    out.extend(b"opus");
    out.extend(8u32.to_le_bytes());
    out.extend(2u32.to_le_bytes());
    out.extend(312u32.to_le_bytes());
    out.extend(b"data");
    out.extend((2u32 * 1920 * 4).to_le_bytes());
    assert_eq!(out.len(), 0x94);
    for packet in &workbench_snd().packets {
        out.extend((packet.len() as u16).to_le_bytes());
        out.extend(packet);
    }
    out
}

#[test]
fn snd_workbench_layout_test() {
    let bytes = workbench_bytes();
    let snd = read(&bytes).unwrap();
    assert_eq!(snd, workbench_snd());
    assert_eq!(write(&snd), bytes);
    assert_eq!(snd.format.format_tag, WAVE_FORMAT_PCM);
    assert_eq!(snd.sample_count(), 3840);
    assert_eq!(snd.duration().as_millis(), 80);
}

/// `MU_MU_Spectre_Basso.snd`: version 1 in the size field, no `ENV `.
#[test]
fn snd_inline_version_without_envelope_test() {
    let mut snd = workbench_snd();
    snd.version = SndVersion::Inline(1);
    snd.envelope = None;
    let bytes = write(&snd);
    assert_eq!(&bytes[12..24], b"VERS\x01\x00\x00\x00fmt ");
    let header_len = 12 + 8 + 24 + 16 + 8;
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        header_len + snd.data_size
    );
    assert_eq!(read(&bytes).unwrap(), snd);

    snd.version = SndVersion::Inline(3);
    assert_eq!(read(&write(&snd)).unwrap(), snd);
}

/// The 64 byte `ENV ` of version 2, without the option byte.
#[test]
fn snd_envelope_without_option_test() {
    let mut snd = workbench_snd();
    snd.version = SndVersion::Inline(2);
    snd.envelope.as_mut().unwrap().dynamic_volume = None;
    let bytes = write(&snd);
    let env = bytes.windows(4).position(|w| w == b"ENV ").unwrap();
    assert_eq!(bytes[env + 4..env + 8], 64u32.to_le_bytes());
    assert_eq!(read(&bytes).unwrap(), snd);
}

#[test]
fn snd_errors_test() {
    let bytes = workbench_bytes();

    let mut wave = bytes.clone();
    wave[8..12].copy_from_slice(b"WAVE");
    assert_eq!(
        snd_error(read(&wave)),
        SndError::UnknownFormType("WAVE".into())
    );

    let mut form = bytes.clone();
    form[0..4].copy_from_slice(b"FORM");
    assert_eq!(snd_error(read(&form)), SndError::NotRiff);

    // The last packet is cut short.
    assert_eq!(
        snd_error(read(&bytes[..bytes.len() - 1])),
        SndError::Truncated
    );
    assert_eq!(snd_error(read(&bytes[..0x50])), SndError::Truncated);

    let mut unknown = bytes.clone();
    unknown[0x7C..0x80].copy_from_slice(b"LIST");
    assert_eq!(
        snd_error(read(&unknown)),
        SndError::UnknownChunk("LIST".into())
    );

    // A second `fmt ` instead of the `ENV `.
    let mut duplicate = bytes[..0x30].to_vec();
    duplicate.extend_from_slice(&bytes[0x18..0x30]);
    duplicate.extend_from_slice(&bytes[0x7C..]);
    assert_eq!(
        snd_error(read(&duplicate)),
        SndError::DuplicateChunk("fmt ")
    );

    let mut missing = bytes[..0x7C].to_vec();
    missing.extend_from_slice(&bytes[0x8C..]);
    assert_eq!(snd_error(read(&missing)), SndError::ChunkMissing("opus"));

    let mut size = bytes.clone();
    size[0x80..0x84].copy_from_slice(&4u32.to_le_bytes());
    assert_eq!(
        snd_error(read(&size)),
        SndError::ChunkSize {
            tag: "opus",
            size: 4
        }
    );

    let mut snd = workbench_snd();
    snd.packets[1] = packet(0x1_0000);
    assert_eq!(
        snd_error(snd.write(&mut Vec::new())),
        SndError::PacketTooLarge(1)
    );
}

#[test]
fn snd_envelope_from_pcm_test() {
    // 64 stereo frames: 2 per level. Level i has the frames (i, -i) and (2i, 0): mean squares
    // i^2 and 2i^2 (truncated), RMS sqrt(1.5) * i.
    let mut samples = Vec::new();
    for i in 0..32i16 {
        samples.extend([i * 100, -i * 100, i * 200, 0]);
    }
    // A frame after the last full part is dropped.
    samples.extend([i16::MAX, i16::MAX]);
    let envelope = SndEnvelope::from_pcm(&samples, 2, false);
    assert_eq!(envelope.dynamic_volume, Some(false));
    for (i, &level) in envelope.levels.iter().enumerate() {
        let square = (i as u64 * 100).pow(2);
        assert_eq!(level, ((3 * square) as f32 / 2.0).sqrt() as u16);
    }

    let loud = SndEnvelope::from_pcm(&[i16::MIN; 64], 1, true);
    assert_eq!(loud.levels, [32768; 32]);

    // Fewer than 32 frames.
    assert_eq!(SndEnvelope::from_pcm(&[1000; 31], 1, true).levels, [0; 32]);
}

#[test]
fn snd_packet_samples_test() {
    // SILK 60 ms, 1 frame.
    assert_eq!(packet_samples(&[0x18]), Some(2880));
    // Hybrid 20 ms, 2 frames.
    assert_eq!(packet_samples(&[0x69]), Some(1920));
    // CELT 2.5 ms, code 3 with 5 frames.
    assert_eq!(packet_samples(&[0x83, 0x05]), Some(600));
    assert_eq!(packet_samples(&[0xFD, 0xFF]), Some(1920));
    assert_eq!(packet_samples(&[0x83]), None);
    assert_eq!(packet_samples(&[]), None);
}

struct OggPage {
    flags: u8,
    granule: u64,
    sequence: u32,
    lacing: Vec<u8>,
    body: Vec<u8>,
}

/// Splits Ogg pages and checks their CRC.
fn ogg_pages(bytes: &[u8]) -> Vec<OggPage> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0u32;
        for &b in bytes {
            crc ^= u32::from(b) << 24;
            for _ in 0..8 {
                crc = if crc & 0x8000_0000 != 0 {
                    (crc << 1) ^ 0x04C1_1DB7
                } else {
                    crc << 1
                };
            }
        }
        crc
    }
    // The check value of CRC-32 with the Ogg parameters.
    assert_eq!(crc32(b"123456789"), 0x89A1_897F);

    let mut pages = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        assert_eq!(&bytes[pos..pos + 5], b"OggS\0");
        let segments = usize::from(bytes[pos + 26]);
        let lacing = bytes[pos + 27..pos + 27 + segments].to_vec();
        let body_len: usize = lacing.iter().map(|&l| usize::from(l)).sum();
        let end = pos + 27 + segments + body_len;
        let mut page = bytes[pos..end].to_vec();
        let crc = u32::from_le_bytes(page[22..26].try_into().unwrap());
        page[22..26].fill(0);
        assert_eq!(crc32(&page), crc);
        assert_eq!(page[14..18], 7u32.to_le_bytes(), "serial");
        pages.push(OggPage {
            flags: page[5],
            granule: u64::from_le_bytes(page[6..14].try_into().unwrap()),
            sequence: u32::from_le_bytes(page[18..22].try_into().unwrap()),
            lacing,
            body: page[27 + segments..].to_vec(),
        });
        pos = end;
    }
    pages
}

/// The packets of Ogg pages, with the granule of the page each packet ends on.
fn ogg_packets(pages: &[OggPage]) -> Vec<(Vec<u8>, u64)> {
    let mut packets = Vec::new();
    let mut packet = Vec::new();
    for page in pages {
        assert_eq!(page.flags & 1 != 0, !packet.is_empty(), "continued flag");
        let mut body = page.body.as_slice();
        for &len in &page.lacing {
            packet.extend(&body[..usize::from(len)]);
            body = &body[usize::from(len)..];
            if len < 255 {
                packets.push((std::mem::take(&mut packet), page.granule));
            }
        }
    }
    assert!(packet.is_empty());
    packets
}

#[test]
fn snd_ogg_test() {
    let snd = workbench_snd();
    let mut ogg = Vec::new();
    snd.write_ogg(&mut ogg, 7).unwrap();
    let pages = ogg_pages(&ogg);
    assert_eq!(pages.len(), 3);
    for (i, page) in pages.iter().enumerate() {
        assert_eq!(page.sequence, i as u32);
    }
    assert_eq!(pages[0].flags, 2);
    assert_eq!(
        pages[0].body,
        b"OpusHead\x01\x02\x38\x01\x80\xbb\x00\x00\x00\x00\x00"
    );
    assert!(pages[1].body.starts_with(b"OpusTags"));
    assert_eq!(pages[2].flags, 4);
    // Ends after the 2 packets of sound, not the flush packet.
    assert_eq!(pages[2].granule, 312 + 3840);

    let packets = ogg_packets(&pages[2..]);
    assert_eq!(
        packets.into_iter().map(|p| p.0).collect::<Vec<_>>(),
        snd.packets
    );
}

/// Packets across pages: a full page of lacing values, a packet of a multiple of 255 bytes
/// and one longer than a page.
#[test]
fn snd_ogg_paging_test() {
    let mut snd = workbench_snd();
    snd.packets = vec![packet(65535), packet(255 * 3), packet(10)];
    snd.packets.extend((0..300).map(|i| packet(1 + i % 3)));
    snd.packet_count = snd.packets.len() as u32 - 1;
    snd.data_size = snd.packet_count * 1920 * 4;
    let mut ogg = Vec::new();
    snd.write_ogg(&mut ogg, 7).unwrap();
    let pages = ogg_pages(&ogg);
    for (i, page) in pages.iter().enumerate() {
        assert_eq!(page.sequence, i as u32);
        assert_eq!(page.flags & 4 != 0, i == pages.len() - 1);
    }
    assert!(pages.iter().any(|p| p.lacing.len() == 255));
    // The 65535 byte packet fills the first audio page and ends on the next one.
    assert_eq!(pages[2].lacing, [255; 255]);
    assert_eq!(pages[2].granule, u64::MAX);
    assert_eq!(pages[3].flags & 1, 1);

    let packets = ogg_packets(&pages[2..]);
    assert_eq!(packets.len(), snd.packets.len());
    let mut granule = 312;
    for (i, ((data, page_granule), original)) in packets.iter().zip(&snd.packets).enumerate() {
        assert_eq!(data, original);
        granule += 1920;
        // The page granule is that of its last packet, at least this one's.
        if i + 1 < packets.len() {
            assert!(*page_granule >= granule);
        }
    }
    assert_eq!(
        pages.last().unwrap().granule,
        312 + u64::from(snd.sample_count())
    );
}

#[test]
fn snd_ogg_errors_test() {
    let mut snd = workbench_snd();
    snd.format = SndFormat::pcm16(6, 48000);
    assert_eq!(
        snd_error(snd.write_ogg(&mut Vec::new(), 0)),
        SndError::UnsupportedChannels(6)
    );

    let mut snd = workbench_snd();
    snd.pre_skip = 70000;
    assert_eq!(
        snd_error(snd.write_ogg(&mut Vec::new(), 0)),
        SndError::PreSkipTooLarge(70000)
    );

    let mut snd = workbench_snd();
    snd.packets[1].clear();
    assert_eq!(
        snd_error(snd.write_ogg(&mut Vec::new(), 0)),
        SndError::InvalidPacket(1)
    );
}

const GAME_DIR: &str = "./tests/enfusion/test-data/snd_in/";

fn game_sound(name: &str) -> (Vec<u8>, Snd) {
    let bytes = std::fs::read(format!("{GAME_DIR}{name}.snd")).unwrap();
    let snd = read(&bytes).unwrap();
    (bytes, snd)
}

#[test]
fn snd_game_round_trip_test() {
    let mut count = 0;
    for entry in std::fs::read_dir(GAME_DIR).unwrap() {
        let path = entry.unwrap().path();
        let bytes = std::fs::read(&path).unwrap();
        let snd = read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(write(&snd), bytes, "{}", path.display());

        assert_eq!(snd.version, SndVersion::Byte(SND_VERSION));
        assert_eq!(snd.format, SndFormat::pcm16(2, 48000));
        assert_eq!(snd.pre_skip, 312);
        assert_eq!(snd.packets.len(), snd.packet_count as usize + 1);
        assert_eq!(snd.sample_count(), snd.packet_count * 1920);
        for packet in &snd.packets {
            assert_eq!(packet_samples(packet), Some(1920));
        }
        // Workbench's flush packet of silence.
        assert_eq!(snd.packets.last().unwrap(), &[0xFD, 0xFF, 0xFE, 0xFF, 0xFE]);

        let mut ogg = Vec::new();
        snd.write_ogg(&mut ogg, 7).unwrap();
        let pages = ogg_pages(&ogg);
        assert_eq!(
            ogg_packets(&pages[2..]).len(),
            snd.packets.len(),
            "{}",
            path.display()
        );
        count += 1;
    }
    assert_eq!(count, 2);
}

/// `Sounds/Music/Campaign/Samples/Jingles/MU_Jingle_Win_01.snd`
#[test]
fn snd_game_jingle_test() {
    let (_, snd) = game_sound("MU_Jingle_Win_01");
    assert_eq!(snd.packet_count, 46);
    assert_eq!(snd.data_size, 46 * 1920 * 4);
    assert_eq!(snd.duration().as_millis(), 1840);
    let envelope = snd.envelope.unwrap();
    assert_eq!(envelope.dynamic_volume, Some(false));
    assert_eq!(envelope.levels[..3], [1706, 2478, 2964]);
    assert_eq!(envelope.levels[31], 5);
    assert_eq!(snd.packets[0].len(), 396);
    assert_eq!(snd.packets[0][0], 0xFE);
}

/// `Sounds/Music/MainMenu/Samples/MU_TheHumanSea_Full_02.snd`, one of two game files with
/// "Dynamic Volume" on.
#[test]
fn snd_game_dynamic_volume_test() {
    let (_, snd) = game_sound("MU_TheHumanSea_Full_02");
    assert_eq!(snd.packet_count, 1501);
    assert_eq!(snd.data_size, 0xAF_E600);
    let envelope = snd.envelope.unwrap();
    assert_eq!(envelope.dynamic_volume, Some(true));
    assert_eq!(envelope.levels[..2], [824, 1189]);
    assert_eq!(envelope.levels[31], 11);
}
