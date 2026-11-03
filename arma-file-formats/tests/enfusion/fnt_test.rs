//! Tests for the fnt reader and writer.
//!
//! The hand built fonts follow the layout of the engine's loader (versions 2 to 5) and the Font
//! Editor's writer. `fnt_probe_*` read what the Font Editor saved from Roboto (`fnt_probes/`, see
//! its README) and Arial (`test-data/fnt_probes_in/`), `fnt_game_*` the game's 13 fonts from
//! `test-data/fnt_in/`.

use std::io::Cursor;

use arma_file_formats::{
    enfusion::fnt::{
        Fnt, FntChunk, FntColorGlyph, FntGlyph, FntHead, FntKerning, FntLayer, FntMetrics,
        FntRange, FntType,
    },
    errors::{AffError, FntError},
};

fn read(bytes: &[u8]) -> Result<Fnt, AffError> {
    Fnt::from(&mut Cursor::new(bytes))
}

fn write(fnt: &Fnt) -> Vec<u8> {
    let mut out = Vec::new();
    fnt.write(&mut out).unwrap();
    out
}

fn fnt_error(result: Result<impl std::fmt::Debug, AffError>) -> FntError {
    match result {
        Err(AffError::FntError(e)) => e,
        other => panic!("expected an FntError, got {other:?}"),
    }
}

fn form(version: &[u8; 4], chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = version.to_vec();
    for (tag, data) in chunks {
        body.extend(*tag);
        body.extend((data.len() as u32).to_be_bytes());
        body.extend(data);
    }
    let mut out = b"FORM".to_vec();
    out.extend((body.len() as u32).to_be_bytes());
    out.extend(body);
    out
}

fn le16(out: &mut Vec<u8>, values: &[u16]) {
    for v in values {
        out.extend(v.to_le_bytes());
    }
}

fn head5() -> Vec<u8> {
    let mut out = 5u32.to_le_bytes().to_vec();
    out.extend(b"Test\0");
    out.extend(28u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.push(2);
    out.extend(38u32.to_le_bytes());
    out.extend(20.0f32.to_le_bytes());
    out.extend(28.0f32.to_le_bytes());
    out.extend(8u16.to_le_bytes());
    out.push(1); // bold
    out.push(0); // italic
    out.extend(27.0f32.to_le_bytes());
    out
}

/// `0x20` and `0x21`, then `0x25A1`, with uninitialized bytes like the Font Editor writes.
fn glps5() -> Vec<u8> {
    let mut out = vec![0xCD; 8];
    out.extend(3u32.to_le_bytes());
    out.extend(2u32.to_le_bytes());
    out.extend(0x20u32.to_le_bytes());
    le16(&mut out, &[2, 0xCDCD]);
    out.extend(0x25A1u32.to_le_bytes());
    le16(&mut out, &[1, 0xCDCD]);
    out
}

fn tcrd5() -> Vec<u8> {
    let mut out = Vec::new();
    le16(&mut out, &[1, 1, 0, 0, 0, 0, 17]);
    le16(&mut out, &[41, 1, 4, 20, 6, 20, 17]);
    le16(&mut out, &[81, 1, 13, 22, 2, (-1i16) as u16, 17]);
    out
}

fn lyrs() -> Vec<u8> {
    let mut out = 1u32.to_le_bytes().to_vec();
    out.extend(24u32.to_le_bytes());
    for v in [0x21u32, 2, 10, 11, 12, 13] {
        out.extend(v.to_le_bytes());
    }
    out
}

fn kern() -> Vec<u8> {
    let mut out = Vec::new();
    le16(&mut out, &[0x20, 0x21]);
    out.extend((-2i32).to_le_bytes());
    out
}

fn fnt5_bytes() -> Vec<u8> {
    form(
        b"FNT5",
        &[
            (b"HEAD", head5()),
            (b"GLPS", glps5()),
            (b"TCRD", tcrd5()),
            (b"LYRS", lyrs()),
            (b"KERN", kern()),
        ],
    )
}

#[test]
fn fnt5_read_test() {
    let fnt = read(&fnt5_bytes()).unwrap();
    assert_eq!(fnt.version, 5);
    assert_eq!(
        fnt.head,
        FntHead {
            name: "Test".into(),
            size: 28,
            y_offset: 0,
            metrics: Some(FntMetrics {
                font_type: FntType::Msdf,
                cell_size: 38,
                cap_height: 20.0,
                y_ppem: 28.0,
                sdf_range: 8,
                bold: true,
                italic: false,
                x_ppem: 27.0,
            }),
        }
    );
    assert_eq!(fnt.glyphs_unused, [0xCD; 8]);
    assert_eq!(
        fnt.ranges,
        [
            FntRange {
                first: 0x20,
                count: 2,
                padding: 0xCDCD
            },
            FntRange {
                first: 0x25A1,
                count: 1,
                padding: 0xCDCD
            },
        ]
    );
    assert_eq!(fnt.glyphs.len(), 3);
    assert_eq!(
        fnt.glyphs[2],
        FntGlyph {
            x: 81,
            y: 1,
            width: 13,
            height: 22,
            offset_x: 2,
            offset_y: -1,
            advance: 17,
        }
    );
    assert_eq!(
        fnt.color_glyphs,
        [FntColorGlyph {
            code_point: 0x21,
            layers: vec![
                FntLayer {
                    code_point: 10,
                    color: 11
                },
                FntLayer {
                    code_point: 12,
                    color: 13
                },
            ],
        }]
    );
    assert_eq!(fnt.color_glyph(0x21), fnt.color_glyphs.first());
    assert_eq!(fnt.color_glyph(0x20), None);
    assert_eq!(
        fnt.kerning,
        [FntKerning {
            first: 0x20,
            second: 0x21,
            amount: -2,
        }]
    );
    assert_eq!(
        fnt.chunk_order,
        [
            FntChunk::Head,
            FntChunk::Glyphs,
            FntChunk::TexCoords,
            FntChunk::Layers,
            FntChunk::Kerning,
        ]
    );
    assert_eq!(fnt.editor_chunk_order(), fnt.chunk_order);
}

#[test]
fn fnt5_round_trip_test() {
    let bytes = fnt5_bytes();
    assert_eq!(write(&read(&bytes).unwrap()), bytes);
}

#[test]
fn fnt_lookup_test() {
    let fnt = read(&fnt5_bytes()).unwrap();
    assert_eq!(fnt.code_points().collect::<Vec<_>>(), [0x20, 0x21, 0x25A1]);
    assert_eq!(fnt.glyph(0x21), Some(&fnt.glyphs[1]));
    assert_eq!(fnt.glyph(0x25A1), Some(&fnt.glyphs[2]));
    assert_eq!(fnt.glyph(0x22), None);
    assert_eq!(fnt.glyph(0x1F), None);
    assert_eq!(fnt.kerning(0x20, 0x21), Some(-2));
    assert_eq!(fnt.kerning(0x21, 0x20), None);
    assert_eq!(fnt.kerning(0x1_0020, 0x21), None);

    // Centered in the 38 pixel cell: (38 - 4) / 2 = 17, (38 - 20) / 2 = 9.
    assert_eq!(fnt.atlas_rect(&fnt.glyphs[1]), [58.0, 10.0, 62.0, 30.0]);
    let mut bitmap = fnt.clone();
    bitmap.head.metrics.as_mut().unwrap().font_type = FntType::Bitmap;
    assert_eq!(bitmap.atlas_rect(&fnt.glyphs[1]), [41.0, 1.0, 45.0, 21.0]);
}

/// The chunk order of the game's FNT3 fonts, which have no metrics and `u16` ranges.
fn fnt3_bytes() -> Vec<u8> {
    let mut glps = vec![4, 1, 0, 0, 0, 0, 0, 0];
    glps.extend(2u32.to_le_bytes());
    glps.extend(1u32.to_le_bytes());
    le16(&mut glps, &[0x41, 2]);
    let mut tcrd = Vec::new();
    le16(&mut tcrd, &[0, 0, 7, 12, 0, 1]);
    le16(&mut tcrd, &[8, 0, 6, 12, 1, (-1i16) as u16]);
    let mut head = 11u32.to_le_bytes().to_vec();
    head.extend(b"METRONBOOK\0");
    head.extend(12u32.to_le_bytes());
    head.extend(0u32.to_le_bytes());
    form(
        b"FNT3",
        &[
            (b"GLPS", glps),
            (b"TCRD", tcrd),
            (b"KERN", Vec::new()),
            (b"HEAD", head),
        ],
    )
}

#[test]
fn fnt3_test() {
    let bytes = fnt3_bytes();
    let fnt = read(&bytes).unwrap();
    assert_eq!(fnt.version, 3);
    assert_eq!(fnt.head.name, "METRONBOOK");
    assert_eq!(fnt.head.size, 12);
    assert_eq!(fnt.head.metrics, None);
    assert_eq!(fnt.glyphs_unused, [4, 1, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        fnt.ranges,
        [FntRange {
            first: 0x41,
            count: 2,
            padding: 0
        }]
    );
    assert_eq!(
        fnt.glyphs[1],
        FntGlyph {
            x: 8,
            y: 0,
            width: 6,
            height: 12,
            offset_x: 1,
            offset_y: -1,
            advance: 0,
        }
    );
    assert!(fnt.kerning.is_empty());
    assert_eq!(
        fnt.chunk_order,
        [
            FntChunk::Glyphs,
            FntChunk::TexCoords,
            FntChunk::Kerning,
            FntChunk::Head
        ]
    );
    assert_eq!(write(&fnt), bytes);

    let mut editor = fnt.clone();
    editor.chunk_order = fnt.editor_chunk_order();
    let reread = read(&write(&editor)).unwrap();
    assert_eq!(reread.chunk_order[0], FntChunk::Head);
    assert_eq!(reread, editor);
}

/// Versions 2 and 4 are not in the game data, the layout follows the engine's loader.
#[test]
fn fnt2_fnt4_test() {
    let fnt3 = read(&fnt3_bytes()).unwrap();

    let mut fnt2 = fnt3.clone();
    fnt2.version = 2;
    for g in &mut fnt2.glyphs {
        (g.offset_x, g.offset_y) = (0, 0);
    }
    let bytes = write(&fnt2);
    assert_eq!(&bytes[8..12], b"FNT2");
    // HEAD 8 + 23, GLPS 8 + 20, TCRD 8 + 2 * 8, KERN 8.
    assert_eq!(bytes.len(), 12 + 31 + 28 + 24 + 8);
    assert_eq!(read(&bytes).unwrap(), fnt2);

    let mut fnt4 = fnt3;
    fnt4.version = 4;
    fnt4.head.metrics = Some(FntMetrics {
        font_type: FntType::Sdf,
        cell_size: 20,
        cap_height: 9.0,
        y_ppem: 12.0,
        sdf_range: 4,
        bold: false,
        italic: true,
        x_ppem: 12.0,
    });
    fnt4.glyphs[0].advance = 8;
    let bytes = write(&fnt4);
    // HEAD 8 + 23 + 21 bytes of metrics, GLPS keeps the u16 ranges (8 + 20), TCRD has 14 byte
    // glyphs (8 + 2 * 14), KERN 8.
    assert_eq!(bytes.len(), 12 + 52 + 28 + 36 + 8);
    assert_eq!(read(&bytes).unwrap(), fnt4);
}

#[test]
fn fnt_write_errors_test() {
    let fnt5 = read(&fnt5_bytes()).unwrap();
    let fnt3 = read(&fnt3_bytes()).unwrap();
    let unwritable = |fnt: &Fnt| fnt_error(fnt.write(&mut Vec::new()));

    let mut f = fnt3.clone();
    f.version = 2;
    assert_eq!(unwritable(&f), FntError::Unwritable("TCRD"));

    let mut f = fnt3.clone();
    f.glyphs[0].advance = 1;
    assert_eq!(unwritable(&f), FntError::Unwritable("TCRD"));

    let mut f = fnt3.clone();
    f.glyphs.pop();
    assert_eq!(unwritable(&f), FntError::Unwritable("TCRD"));

    let mut f = fnt3.clone();
    f.ranges[0].first = 0x1_0000;
    assert_eq!(unwritable(&f), FntError::Unwritable("GLPS"));

    let mut f = fnt3.clone();
    f.head.metrics = fnt5.head.metrics;
    assert_eq!(unwritable(&f), FntError::Unwritable("HEAD"));

    let mut f = fnt5.clone();
    f.head.metrics = None;
    assert_eq!(unwritable(&f), FntError::Unwritable("HEAD"));

    let mut f = fnt5.clone();
    f.version = 6;
    assert_eq!(unwritable(&f), FntError::Unwritable("version"));

    let mut f = fnt5.clone();
    f.chunk_order.retain(|c| *c != FntChunk::Kerning);
    assert_eq!(unwritable(&f), FntError::Unwritable("KERN"));
    f.kerning.clear();
    f.color_glyphs.clear();
    f.chunk_order.retain(|c| *c != FntChunk::Layers);
    assert_eq!(f.editor_chunk_order().last(), Some(&FntChunk::Kerning));
    read(&write(&f)).unwrap();

    let mut f = fnt5.clone();
    f.chunk_order.retain(|c| *c != FntChunk::Head);
    assert_eq!(unwritable(&f), FntError::ChunkMissing("HEAD"));

    let mut f = fnt5;
    f.chunk_order.push(FntChunk::Kerning);
    assert_eq!(unwritable(&f), FntError::DuplicateChunk("KERN".into()));
}

#[test]
fn fnt_read_errors_test() {
    let all = |replace: Option<(&[u8; 4], Vec<u8>)>, extra: Option<(&[u8; 4], Vec<u8>)>| {
        let mut chunks = vec![(b"HEAD", head5()), (b"GLPS", glps5()), (b"TCRD", tcrd5())];
        if let Some((tag, data)) = replace {
            chunks.iter_mut().find(|c| c.0 == tag).unwrap().1 = data;
        }
        chunks.extend(extra);
        read(&form(b"FNT5", &chunks))
    };

    assert!(matches!(
        read(b"RIFF\0\0\0\x04FNT5"),
        Err(AffError::InvalidFileError)
    ));
    for form_type in [b"FONT", b"FNT1", b"FNT6"] {
        assert_eq!(
            fnt_error(read(&form(form_type, &[]))),
            FntError::UnknownFormType(String::from_utf8_lossy(form_type).into())
        );
    }
    assert_eq!(
        fnt_error(read(&form(b"FNT5", &[(b"HEAD", head5())]))),
        FntError::ChunkMissing("GLPS")
    );
    assert_eq!(
        fnt_error(all(None, Some((b"XXXX", Vec::new())))),
        FntError::UnknownChunk("XXXX".into())
    );
    assert_eq!(
        fnt_error(all(None, Some((b"TCRD", tcrd5())))),
        FntError::DuplicateChunk("TCRD".into())
    );

    let mut bytes = fnt5_bytes();
    bytes.truncate(bytes.len() - 1);
    assert_eq!(fnt_error(read(&bytes)), FntError::Truncated);
    // The form size covers a cut chunk.
    let mut bytes = fnt5_bytes();
    let len = bytes.len();
    bytes.truncate(len - 1);
    bytes[4..8].copy_from_slice(&(len as u32 - 9).to_be_bytes());
    assert_eq!(fnt_error(read(&bytes)), FntError::Truncated);

    let mut head = head5();
    head[4 + 5 + 8] = 3;
    assert_eq!(
        fnt_error(all(Some((b"HEAD", head)), None)),
        FntError::UnknownFontType(3)
    );
    let mut head = head5();
    let bold = head.len() - 6;
    head[bold] = 2;
    assert_eq!(
        fnt_error(all(Some((b"HEAD", head)), None)),
        FntError::InvalidChunk("HEAD")
    );
    let mut head = head5();
    head.push(0);
    assert_eq!(
        fnt_error(all(Some((b"HEAD", head)), None)),
        FntError::InvalidChunk("HEAD")
    );
    let mut head = head5();
    head[8] = b'x'; // the NUL of the name
    assert_eq!(
        fnt_error(all(Some((b"HEAD", head)), None)),
        FntError::InvalidChunk("HEAD")
    );

    let mut glps = glps5();
    glps[8] = 4; // glyph count
    assert_eq!(
        fnt_error(all(Some((b"GLPS", glps)), None)),
        FntError::InvalidChunk("GLPS")
    );
    let mut glps = glps5();
    glps[12] = 200; // range count
    assert_eq!(
        fnt_error(all(Some((b"GLPS", glps)), None)),
        FntError::InvalidChunk("GLPS")
    );

    let mut tcrd = tcrd5();
    tcrd.truncate(28);
    assert_eq!(
        fnt_error(all(Some((b"TCRD", tcrd)), None)),
        FntError::InvalidChunk("TCRD")
    );
    let mut tcrd = tcrd5();
    tcrd.pop();
    assert_eq!(
        fnt_error(all(Some((b"TCRD", tcrd)), None)),
        FntError::InvalidChunk("TCRD")
    );

    assert_eq!(
        fnt_error(all(None, Some((b"KERN", vec![0; 7])))),
        FntError::InvalidChunk("KERN")
    );
    let mut layers = lyrs();
    layers[4] = 28; // entry size disagrees with the layer count
    layers.extend([0; 4]);
    assert_eq!(
        fnt_error(all(None, Some((b"LYRS", layers)))),
        FntError::InvalidChunk("LYRS")
    );
}

const GAME_DIR: &str = "./tests/enfusion/test-data/fnt_in/";

fn game_font(name: &str) -> (Vec<u8>, Fnt) {
    let bytes = std::fs::read(format!("{GAME_DIR}{name}.fnt")).unwrap();
    let fnt = read(&bytes).unwrap();
    (bytes, fnt)
}

/// All fonts of the game (`UI/Fonts/**`, `ui/fonts/robotomono_msdf_28.fnt` in core).
#[test]
fn fnt_game_round_trip_test() {
    let mut count = 0;
    for entry in std::fs::read_dir(GAME_DIR).unwrap() {
        let path = entry.unwrap().path();
        let bytes = std::fs::read(&path).unwrap();
        let fnt = read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(write(&fnt), bytes, "{}", path.display());
        assert!(fnt.color_glyphs.is_empty() && fnt.kerning.is_empty());
        // The engine refuses distance field fonts without these.
        if fnt.version == 5 {
            assert!(fnt.glyph(0x20).is_some() && fnt.glyph(0x25A1).is_some());
        }
        count += 1;
    }
    assert_eq!(count, 13);
}

#[test]
fn fnt_game_msdf_test() {
    let (_, fnt) = game_font("robotomono_msdf_28");
    assert_eq!(fnt.version, 5);
    assert_eq!(fnt.head.name, "robotomono_msdf_28");
    assert_eq!(fnt.head.size, 28);
    assert_eq!(
        fnt.head.metrics,
        Some(FntMetrics {
            font_type: FntType::Msdf,
            cell_size: 38,
            cap_height: 20.0,
            y_ppem: 28.0,
            sdf_range: 8,
            bold: false,
            italic: false,
            x_ppem: 28.0,
        })
    );
    assert_eq!(fnt.glyphs_unused, [0xCD; 8]);
    assert_eq!(fnt.glyphs.len(), 127);
    assert_eq!(fnt.ranges.len(), 23);
    assert!(fnt.ranges.iter().all(|r| r.padding == 0xCDCD));
    assert_eq!(
        fnt.ranges[0],
        FntRange {
            first: 0x20,
            count: 95,
            padding: 0xCDCD
        }
    );
    assert_eq!(fnt.code_points().last(), Some(0x25A1));
    let bang = fnt.glyph('!' as u32).unwrap();
    assert_eq!(
        *bang,
        FntGlyph {
            x: 41,
            y: 1,
            width: 4,
            height: 20,
            offset_x: 6,
            offset_y: 20,
            advance: 17,
        }
    );
    // Checked against the atlas `ui/fonts/robotomono_msdf_28.edds`: the ink of `!` is there.
    assert_eq!(fnt.atlas_rect(bang), [58.0, 10.0, 62.0, 30.0]);
    // Monospaced.
    assert!(fnt.glyphs.iter().take(95).all(|g| g.advance == 17));
}

#[test]
fn fnt_game_sdf_test() {
    let (_, fnt) = game_font("Roboto_Bold");
    assert_eq!(fnt.head.name, "Roboto_Bold");
    assert_eq!(
        fnt.head.metrics,
        Some(FntMetrics {
            font_type: FntType::Sdf,
            cell_size: 123,
            cap_height: 57.0,
            y_ppem: 80.0,
            sdf_range: 16,
            bold: false,
            italic: false,
            x_ppem: 80.0,
        })
    );
    assert_eq!(fnt.glyphs.len(), 912);
    assert_eq!(fnt.ranges.len(), 91);
    // Not sorted by code point.
    assert!(fnt.ranges.windows(2).any(|w| w[0].first > w[1].first));
    // The largest glyph plus the SDF range.
    let tallest = fnt.glyphs.iter().map(|g| g.width.max(g.height)).max();
    assert_eq!(tallest, Some(107));

    let (_, noto) = game_font("NotoSans-Enhanced-Regular-JP-KOR-SC-LAT");
    assert_eq!(noto.glyphs.len(), 13747);
    assert_eq!(noto.ranges.len(), 6184);
    assert_eq!(noto.head.metrics.unwrap().cell_size, 66);
    assert!(noto.glyph('日' as u32).is_some());
}

#[test]
fn fnt_game_bitmap_test() {
    for size in [12, 14, 16, 22, 28, 48, 58] {
        let (_, fnt) = game_font(&format!("default{size}"));
        assert_eq!(fnt.version, 3);
        assert_eq!(fnt.head.name, "METRONBOOK");
        assert_eq!(fnt.head.size, size);
        assert_eq!(fnt.head.y_offset, 0);
        assert_eq!(fnt.head.metrics, None);
        assert_eq!(fnt.glyphs.len(), 503);
        assert_eq!(fnt.ranges.len(), 61);
        assert_eq!(
            fnt.chunk_order,
            [
                FntChunk::Glyphs,
                FntChunk::TexCoords,
                FntChunk::Kerning,
                FntChunk::Head
            ]
        );
        // Every glyph is a full line high.
        assert!(fnt.glyphs.iter().all(|g| g.height == size as u16));
    }

    let (_, fnt) = game_font("default28");
    // Checked against the atlas `UI/Fonts/Default/default28.edds`.
    let percent = fnt.glyph('%' as u32).unwrap();
    assert_eq!(
        *percent,
        FntGlyph {
            x: 45,
            y: 0,
            width: 19,
            height: 28,
            offset_x: 0,
            offset_y: 1,
            advance: 0,
        }
    );
    assert_eq!(fnt.atlas_rect(percent), [45.0, 0.0, 64.0, 28.0]);
}

fn probe(path: &str) -> Fnt {
    let bytes = std::fs::read(path).unwrap();
    let fnt = read(&bytes).unwrap();
    assert_eq!(write(&fnt), bytes, "{path}");
    fnt
}

#[test]
fn fnt_probe_bitmap_test() {
    let fnt = probe("./tests/enfusion/fnt_probes/roboto_bitmap.fnt");
    // The Font Editor writes bitmap fonts as FNT3, with its own chunk order.
    assert_eq!(fnt.version, 3);
    assert_eq!(fnt.chunk_order, fnt.editor_chunk_order());
    assert_eq!(fnt.head.name, "roboto_bitmap_3224");
    assert_eq!(fnt.head.size, 24);
    assert_eq!(fnt.head.metrics, None);
    assert_eq!(
        fnt.ranges,
        [
            FntRange {
                first: 0x20,
                count: 95,
                padding: 0
            },
            FntRange {
                first: 0x25A1,
                count: 1,
                padding: 0
            },
        ]
    );
    assert!(fnt.glyphs.iter().all(|g| g.height == 26 && g.advance == 0));
    // The space is only space after it, as wide as the SDF font's advance.
    let space = fnt.glyph(0x20).unwrap();
    assert_eq!((space.width, space.offset_x, space.offset_y), (0, 0, 6));
    assert!(fnt.kerning.is_empty());
}

#[test]
fn fnt_probe_sdf_test() {
    let fnt = probe("./tests/enfusion/fnt_probes/roboto_sdf.fnt");
    assert_eq!(fnt.version, 5);
    assert_eq!(fnt.chunk_order, fnt.editor_chunk_order());
    assert_eq!(fnt.head.name, "roboto_sdf_32");
    assert_eq!(fnt.head.size, 24);
    assert_eq!(
        fnt.head.metrics,
        Some(FntMetrics {
            font_type: FntType::Sdf,
            cell_size: 33,
            // Roboto's cap height is 1456 / 2048 em.
            cap_height: 17.0,
            y_ppem: 24.0,
            sdf_range: 8,
            bold: false,
            italic: false,
            x_ppem: 24.0,
        })
    );
    assert_eq!(
        fnt.glyphs_unused,
        [0x40, 0xbc, 0x73, 0x42, 0xfa, 0x7f, 0, 0]
    );
    assert_eq!(fnt.glyphs.len(), 96);
    assert_eq!(fnt.code_points().last(), Some(0x25A1));
    assert_eq!(fnt.glyph(0x20).unwrap().advance, 6);
    assert_eq!(
        fnt.glyph('A' as u32),
        Some(&FntGlyph {
            x: 176,
            y: 71,
            width: 16,
            height: 17,
            offset_x: 0,
            offset_y: 17,
            advance: 16,
        })
    );
    // The cell is the largest glyph plus the SDF range.
    let largest = fnt.glyphs.iter().map(|g| g.width.max(g.height)).max();
    assert_eq!(largest, Some(33 - 8));
    // Roboto has its kerning in GPOS only.
    assert!(fnt.kerning.is_empty());

    // Bold and Italic only set their flag.
    for (name, bold, italic) in [("bold", true, false), ("italic", false, true)] {
        let styled = probe(&format!(
            "./tests/enfusion/fnt_probes/roboto_sdf_{name}.fnt"
        ));
        let metrics = styled.head.metrics.unwrap();
        assert_eq!((metrics.bold, metrics.italic), (bold, italic));
        let mut plain = styled.clone();
        plain.head.name = fnt.head.name.clone();
        plain.head.metrics = fnt.head.metrics;
        assert_eq!(plain, fnt);
    }
}

#[test]
fn fnt_probe_kerning_test() {
    let fnt = probe("./tests/enfusion/test-data/fnt_probes_in/arial_sdf_32.fnt");
    assert_eq!(fnt.kerning.len(), 96);
    // Arial's kern table, 2048 units per em, at 24 pixels: `A V` -152 -> -1.78.
    assert_eq!(fnt.kerning('A' as u32, 'V' as u32), Some(-2));
    assert_eq!(fnt.kerning(' ' as u32, 'A' as u32), Some(-1));
    // -37 -> -0.43 is kept as 0.
    assert_eq!(fnt.kerning(' ' as u32, 'T' as u32), Some(0));
    // `W i` is 0 in the table and left out.
    assert_eq!(fnt.kerning('W' as u32, 'i' as u32), None);
    assert_eq!(
        fnt.kerning[..2],
        [
            FntKerning {
                first: 0x20,
                second: 0x41,
                amount: -1
            },
            FntKerning {
                first: 0x20,
                second: 0x54,
                amount: 0
            },
        ]
    );
}

/// Segoe UI Emoji, Ascii and `U+1F600..=U+1F603`, layers checked against its `COLR` and `CPAL`.
#[test]
fn fnt_probe_color_glyphs_test() {
    let fnt = probe("./tests/enfusion/test-data/fnt_probes_in/emoji2_sdf.fnt");
    assert_eq!(fnt.chunk_order, fnt.editor_chunk_order());
    assert_eq!(fnt.chunk_order[3], FntChunk::Layers);
    assert_eq!(fnt.color_glyphs.len(), 4);

    let grinning = fnt.color_glyph(0x1F600).unwrap();
    let layer = |code_point, color| FntLayer { code_point, color };
    // COLR glyphs 45442, 45439, 45440, 45441, 45443 and 45444, CPAL colours.
    assert_eq!(
        grinning.layers,
        [
            layer(0x11_B181, 0xFFFF_B02E),
            layer(0x11_B17E, 0xFFBB_1D80),
            layer(0x11_B17F, 0xFFFF_FFFF),
            layer(0x11_B180, 0xFFFF_FFFF),
            layer(0x11_B182, 0xFF40_2A32),
            layer(0x11_B183, 0xFF40_2A32),
        ]
    );
    // Every emoji's face is the same shape, they share its glyph.
    assert!(
        fnt.color_glyphs
            .iter()
            .all(|c| c.layers[0] == layer(0x11_B181, 0xFFFF_B02E))
    );
    // Every layer has a glyph without advance, above Unicode.
    for c in &fnt.color_glyphs {
        assert_eq!(fnt.glyph(c.code_point).unwrap().advance, 33);
        for l in &c.layers {
            assert!(l.code_point >= FntLayer::FIRST_CODE_POINT);
            assert_eq!(fnt.glyph(l.code_point).unwrap().advance, 0);
        }
    }
    let layer_glyphs = fnt
        .code_points()
        .filter(|c| *c >= FntLayer::FIRST_CODE_POINT)
        .count();
    assert_eq!(fnt.glyphs.len(), 96 + 4 + layer_glyphs);
}
