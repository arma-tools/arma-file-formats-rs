//! Tests for the `.fxy` font reader and writer.
//!
//! `test-data/fxy_in/` holds Arma 3 fonts of `uifonts_f_data.pbo`: `Purista10.fxy` and
//! `EtelkaNarrowMediumPro10.fxy` (version 1, the second not sorted by character),
//! `Purista-Bold10.fxy` (version 2 with kerning), `EtelkaMonospacePro10.fxy` (version 2 without)
//! and `NotoSansCJK-Light6.fxy` (23 pages). `fonttotga_Arial10.fxy` is FontToTGA's output for
//! `-size=10 Arial` with its default `FontToTga.cfg`.

use std::io::Cursor;

use arma_file_formats::{
    errors::{AffError, FxyError},
    real_virtuality::fxy::{
        EMPTY_GLYPH_OFFSET, FXY_VERSION_1, FXY_VERSION_2, Fxy, FxyCell, FxyGlyph, FxyKerning,
        FxyLetter, FxyPage,
    },
};

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/fxy_in/";

const FILES: [&str; 6] = [
    "Purista10.fxy",
    "EtelkaNarrowMediumPro10.fxy",
    "Purista-Bold10.fxy",
    "EtelkaMonospacePro10.fxy",
    "NotoSansCJK-Light6.fxy",
    "fonttotga_Arial10.fxy",
];

fn input(name: &str) -> Vec<u8> {
    std::fs::read(format!("{INPUT_PATH_PREFIX}{name}")).unwrap()
}

fn read(bytes: &[u8]) -> Result<Fxy, AffError> {
    Fxy::from(&mut Cursor::new(bytes))
}

fn write(fxy: &Fxy) -> Vec<u8> {
    let mut out = Vec::new();
    fxy.write(&mut out).unwrap();
    out
}

fn error(result: Result<impl std::fmt::Debug, AffError>) -> FxyError {
    match result {
        Err(AffError::FxyError(e)) => e,
        other => panic!("expected an FxyError, got {other:?}"),
    }
}

fn pages(fxy: &Fxy) -> &[FxyPage] {
    match fxy {
        Fxy::V2 { pages } => pages,
        Fxy::V1 { .. } => panic!("expected version 2"),
    }
}

fn cells(fxy: &Fxy) -> &[FxyCell] {
    match fxy {
        Fxy::V1 { cells } => cells,
        Fxy::V2 { .. } => panic!("expected version 1"),
    }
}

#[test]
fn fxy_round_trip_test() {
    for name in FILES {
        let bytes = input(name);
        assert_eq!(write(&read(&bytes).unwrap()), bytes, "{name}");
    }
}

#[test]
fn fxy_v1_test() {
    let fxy = read(&input("Purista10.fxy")).unwrap();
    assert_eq!(fxy.version(), FXY_VERSION_1);
    assert_eq!(fxy.metrics(), None);
    let cells = cells(&fxy);
    assert_eq!(cells.len(), 464);
    // Stored as `0`, the space has no pixels.
    assert_eq!(
        cells[0],
        FxyCell {
            code: 0x20,
            page: 1,
            x: 1,
            y: 1,
            width: 0,
            height: 0,
            advance: 0
        }
    );
    assert_eq!(
        fxy.glyph('A' as u32),
        Some(FxyLetter {
            page: 1,
            x: 97,
            y: 37,
            width: 9,
            height: 15,
            x_offset: 0,
            y_offset: 0,
            advance: 9
        })
    );
    // Cyrillic on page 3, this one has no pixels in Purista.
    assert_eq!(
        fxy.glyph('Ж' as u32).map(|g| (g.page, g.width)),
        Some((3, 0))
    );
    assert_eq!(fxy.glyph('™' as u32), None);
    assert_eq!(fxy.kerning('A' as u32, 'V' as u32), 0);
}

#[test]
fn fxy_v1_unsorted_test() {
    let fxy = read(&input("EtelkaNarrowMediumPro10.fxy")).unwrap();
    let cells = cells(&fxy);
    assert_eq!(cells.len(), 518);
    // Grouped by page: page 4 starts with U+02D8 after the Cyrillic of page 3.
    assert_eq!((cells[465].code, cells[465].page), (0x491, 3));
    assert_eq!((cells[466].code, cells[466].page), (0x2d8, 4));
    assert_eq!(
        fxy.glyph(0x2122),
        Some(FxyLetter {
            page: 4,
            x: 37,
            y: 45,
            width: 9,
            height: 8,
            x_offset: 0,
            y_offset: 0,
            advance: 9
        })
    );
}

#[test]
fn fxy_v2_test() {
    let fxy = read(&input("Purista-Bold10.fxy")).unwrap();
    assert_eq!(fxy.version(), FXY_VERSION_2);
    assert_eq!(fxy.metrics(), Some((17, 14)));
    let pages = pages(&fxy);
    let summary: Vec<_> = pages
        .iter()
        .map(|p| (p.page, p.height, p.ascent, p.kerning.len(), p.glyphs.len()))
        .collect();
    assert_eq!(
        summary,
        [
            (1, 17, 14, 501, 224),
            (2, 17, 14, 81, 146),
            (3, 17, 14, 0, 96),
            (4, 17, 14, 60, 52)
        ]
    );
    assert_eq!(
        pages[0].glyphs[0],
        FxyGlyph {
            code: 0x20,
            x: 2,
            y: 2,
            width: 0,
            height: 0,
            x_offset: EMPTY_GLYPH_OFFSET,
            y_offset: EMPTY_GLYPH_OFFSET,
            advance: 4
        }
    );
    assert_eq!(
        pages[0].kerning[0],
        FxyKerning {
            first: ',' as u16,
            second: '1' as u16,
            amount: -1
        }
    );
    // Pairs are on the page of their first character.
    assert_eq!(
        pages[1].kerning[0],
        FxyKerning {
            first: 0x141,
            second: '-' as u16,
            amount: -1
        }
    );
    assert_eq!(fxy.kerning(',' as u32, '1' as u32), -1);
    assert_eq!(fxy.kerning(0x141, 'T' as u32), -1);
    assert_eq!(fxy.kerning('1' as u32, ',' as u32), 0);
    assert_eq!(
        fxy.glyph('A' as u32),
        Some(FxyLetter {
            page: 1,
            x: 89,
            y: 30,
            width: 9,
            height: 9,
            x_offset: 0,
            y_offset: 5,
            advance: 8
        })
    );
    assert_eq!(
        fxy.glyph('™' as u32),
        Some(FxyLetter {
            page: 4,
            x: 97,
            y: 17,
            width: 9,
            height: 5,
            x_offset: 0,
            y_offset: 5,
            advance: 10
        })
    );
}

#[test]
fn fxy_v2_pages_test() {
    let fxy = read(&input("NotoSansCJK-Light6.fxy")).unwrap();
    let pages = pages(&fxy);
    assert_eq!(pages.len(), 23);
    assert!(
        pages
            .iter()
            .enumerate()
            .all(|(i, p)| usize::from(p.page) == i + 1)
    );
    assert!(pages.iter().all(|p| (p.height, p.ascent) == (12, 9)));
    assert_eq!(pages.iter().map(|p| p.glyphs.len()).sum::<usize>(), 15_856);
    // Not sorted: U+2015 comes first.
    assert_eq!(pages[0].glyphs[0].code, 0x2015);
    assert_eq!(
        fxy.glyph(0x4e00).map(|g| (g.page, g.x, g.y)),
        Some((1, 140, 2))
    );
}

#[test]
fn fxy_fonttotga_test() {
    let fxy = read(&input("fonttotga_Arial10.fxy")).unwrap();
    let pages = pages(&fxy);
    let counts: Vec<_> = pages.iter().map(|p| p.glyphs.len()).collect();
    // The ranges of the default config: 0x20..0x100, 0x100..0x180, 0x400..0x460, 0x2010..0x2020
    // and 0x2122, the last page only has the glyphs Arial has.
    assert_eq!(counts, [224, 128, 96, 17]);
    assert_eq!(fxy.metrics(), Some((16, 13)));
    // The bottom of 'A' is the baseline: y offset 3 + height 10 = ascent 13.
    let a = fxy.glyph('A' as u32).unwrap();
    assert_eq!((a.x, a.y, a.width, a.height), (41, 30, 10, 10));
    assert_eq!((a.x_offset, a.y_offset, a.advance), (-1, 3, 9));
    // 'g' reaches the bottom of the line.
    let g = fxy.glyph('g' as u32).unwrap();
    assert_eq!(g.y_offset + i32::from(g.height), 16);
    assert_eq!(fxy.kerning('A' as u32, 'T' as u32), -1);
}

#[test]
fn fxy_empty_test() {
    // FontToTGA without characters writes only the header.
    let bytes = b"BIFo\x02\x01\x00\x00";
    let fxy = read(bytes).unwrap();
    assert_eq!(fxy, Fxy::V2 { pages: vec![] });
    assert_eq!(fxy.metrics(), None);
    assert_eq!(fxy.glyph('A' as u32), None);
    assert_eq!(write(&fxy), bytes);
    let fxy = read(b"BIFo\x01\x01\x00\x00").unwrap();
    assert_eq!(fxy, Fxy::V1 { cells: vec![] });
}

#[test]
fn fxy_errors_test() {
    assert_eq!(
        error(read(b"0DHT\x02\x01\x00\x00")),
        FxyError::UnknownSignature("0DHT".into())
    );
    assert_eq!(
        error(read(b"BIFo\x03\x01\x00\x00")),
        FxyError::UnsupportedVersion(0x103)
    );
    assert_eq!(error(read(b"BIF")), FxyError::Truncated);
    assert_eq!(error(read(b"BIFo\x02\x01")), FxyError::Truncated);

    let bytes = input("Purista10.fxy");
    assert_eq!(error(read(&bytes[..bytes.len() - 1])), FxyError::Truncated);
    let bytes = input("Purista-Bold10.fxy");
    assert_eq!(error(read(&bytes[..bytes.len() - 1])), FxyError::Truncated);
    // Cut inside the kerning of the first page.
    assert_eq!(error(read(&bytes[..30])), FxyError::Truncated);
    // A huge count doesn't allocate before the data runs out.
    let mut huge = b"BIFo\x02\x01\x00\x00".to_vec();
    huge.extend([1, 0, 16, 0, 0, 0, 13, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(error(read(&huge)), FxyError::Truncated);
}

#[test]
fn fxy_write_errors_test() {
    let cell = |code| FxyCell {
        code,
        page: 1,
        x: 0,
        y: 0,
        width: 0,
        height: 0,
        advance: 0,
    };
    let mut out = Vec::new();
    for code in [0x1f, 0x1_0020] {
        let result = Fxy::V1 {
            cells: vec![cell(code)],
        }
        .write(&mut out);
        assert_eq!(error(result), FxyError::InvalidCode(code));
    }
    let fxy = Fxy::V1 {
        cells: vec![cell(0x20), cell(0x1_001f)],
    };
    assert_eq!(
        &write(&fxy)[8..],
        [
            0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0
        ]
    );
}

#[test]
fn fxy_texture_path_test() {
    assert_eq!(
        Fxy::texture_path("Fonts\\Purista\\Purista10.fxy", 1),
        "Fonts\\Purista\\Purista10-01.paa"
    );
    assert_eq!(Fxy::texture_path("NOTO6.FXY", 23), "NOTO6-23.paa");
    assert_eq!(Fxy::texture_path("Purista10", 4), "Purista10-04.paa");
    assert_eq!(Fxy::texture_path("a.fxyz", 1), "a.fxyz-01.paa");
    assert_eq!(Fxy::texture_path("fxy", 100), "fxy-100.paa");
    // Multi-byte characters before the extension.
    assert_eq!(Fxy::texture_path("ž.fxy", 2), "ž-02.paa");
    assert_eq!(Fxy::texture_path("až", 2), "až-02.paa");
}
