//! Tests for the star catalog reader and writer.
//!
//! The hand built catalogs follow the layout of the game's `system/star_database.stars`, which
//! `stars_game_*` read from `test-data/stars_in/`.

use std::io::Cursor;

use arma_file_formats::{
    enfusion::stars::{Star, StarCatalog, StarsChunk},
    errors::{AffError, StarsError},
};

fn read(bytes: &[u8]) -> Result<StarCatalog, AffError> {
    StarCatalog::from(&mut Cursor::new(bytes))
}

fn write(catalog: &StarCatalog) -> Vec<u8> {
    let mut out = Vec::new();
    catalog.write(&mut out).unwrap();
    out
}

fn stars_error(result: Result<impl std::fmt::Debug, AffError>) -> StarsError {
    match result {
        Err(AffError::StarsError(e)) => e,
        other => panic!("expected a StarsError, got {other:?}"),
    }
}

/// A `STAR` form: the count, then the chunks.
fn form(count: u32, chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = b"STAR".to_vec();
    body.extend(count.to_le_bytes());
    for (tag, data) in chunks {
        body.extend_from_slice(*tag);
        body.extend((data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
    }
    let mut out = b"FORM".to_vec();
    out.extend((body.len() as u32).to_be_bytes());
    out.extend(body);
    out
}

/// The `ITEM` contents of a star, `name` with its NUL.
fn item(floats: [f32; 6], color: [u8; 4], hip: u32, name: &[u8]) -> Vec<u8> {
    let mut data: Vec<u8> = floats.iter().flat_map(|f| f.to_le_bytes()).collect();
    data.extend(color);
    data.extend(hip.to_le_bytes());
    data.extend((name.len() as u32).to_le_bytes());
    data.extend_from_slice(name);
    data
}

const SIRIUS: [f32; 6] = [6.752_477, -16.716_116, -546.01, -1223.08, 379.21, -1.44];
const ANONYMOUS: [f32; 6] = [14.660_138, -60.833_977, -3678.19, 481.84, 742.12, -0.01];

fn sirius() -> Star {
    Star {
        right_ascension: SIRIUS[0],
        declination: SIRIUS[1],
        proper_motion_ra: SIRIUS[2],
        proper_motion_dec: SIRIUS[3],
        parallax: SIRIUS[4],
        magnitude: SIRIUS[5],
        color: 0xFFC4_D6FF,
        hip: 32349,
        name: "SIRIUS A".into(),
    }
}

fn sample_bytes() -> Vec<u8> {
    form(
        2,
        &[
            (
                b"ITEM",
                item(SIRIUS, [0xFF, 0xD6, 0xC4, 0xFF], 32349, b"SIRIUS A\0"),
            ),
            (
                b"ITEM",
                item(ANONYMOUS, [0xFE, 0xFE, 0xFF, 0xFF], 71683, b"\0"),
            ),
        ],
    )
}

#[test]
fn stars_read_test() {
    let catalog = read(&sample_bytes()).unwrap();
    assert_eq!(catalog.stars.len(), 2);
    assert_eq!(catalog.stars[0], sirius());
    let star = &catalog.stars[1];
    assert_eq!(star.hip, 71683);
    assert_eq!(star.name, "");
    assert_eq!(star.parallax, 742.12);
    assert!(catalog.unknown_chunks.is_empty());
}

#[test]
fn stars_round_trip_test() {
    let bytes = sample_bytes();
    assert_eq!(write(&read(&bytes).unwrap()), bytes);
}

#[test]
fn stars_unpadded_items_test() {
    // `SIRIUS A\0` gives an odd chunk size, the next chunk follows without a pad byte.
    let bytes = sample_bytes();
    assert_eq!(&bytes[16..24], b"ITEM\0\0\0\x2D");
    assert_eq!(&bytes[16 + 8 + 45..16 + 8 + 45 + 4], b"ITEM");
}

#[test]
fn stars_empty_test() {
    let bytes = write(&StarCatalog::default());
    assert_eq!(bytes, form(0, &[]));
    assert_eq!(read(&bytes).unwrap(), StarCatalog::default());
}

#[test]
fn stars_color_test() {
    let mut star = sirius();
    assert_eq!(star.rgba(), [0xC4, 0xD6, 0xFF, 0xFF]);
    star.set_rgba([1, 2, 3, 4]);
    assert_eq!(star.color, 0x0401_0203);
    assert_eq!(star.rgba(), [1, 2, 3, 4]);
}

#[test]
fn stars_brightness_test() {
    let mut star = sirius();
    star.magnitude = 0.0;
    assert_eq!(star.brightness(), 1.0);
    star.magnitude = 5.0;
    assert!((star.brightness() - 0.01).abs() < 1e-6);
    // The engine's constant, 2.5118864^-mag.
    star.magnitude = -1.44;
    assert!((star.brightness() - 2.511_886_4f32.powf(1.44)).abs() < 1e-5);
}

#[test]
fn stars_find_and_sort_test() {
    let mut catalog = read(&sample_bytes()).unwrap();
    assert_eq!(catalog.find("sirius a").unwrap().hip, 32349);
    assert!(catalog.find("").is_none());
    assert!(catalog.find("VEGA").is_none());

    catalog.stars.reverse();
    catalog.sort_by_magnitude();
    assert_eq!(catalog.stars[0].hip, 32349);
    assert_eq!(write(&catalog), sample_bytes());
}

#[test]
fn stars_unknown_chunk_test() {
    let sirius_item = item(SIRIUS, [0xFF, 0xD6, 0xC4, 0xFF], 32349, b"SIRIUS A\0");
    let bytes = form(
        1,
        &[(b"ITEM", sirius_item.clone()), (b"XTRA", vec![1, 2, 3])],
    );
    let catalog = read(&bytes).unwrap();
    assert_eq!(
        catalog.unknown_chunks,
        [StarsChunk {
            tag: *b"XTRA",
            data: vec![1, 2, 3]
        }]
    );
    assert_eq!(write(&catalog), bytes);

    // Unknown chunks are written after the stars.
    let before = form(1, &[(b"XTRA", vec![1, 2, 3]), (b"ITEM", sirius_item)]);
    assert_eq!(write(&read(&before).unwrap()), bytes);
}

#[test]
fn stars_form_type_test() {
    let mut bytes = sample_bytes();
    bytes[8..12].copy_from_slice(b"STAX");
    assert_eq!(
        stars_error(read(&bytes)),
        StarsError::UnknownFormType("STAX".into())
    );
    assert!(matches!(
        read(b"RIFF1234STAR"),
        Err(AffError::InvalidFileError)
    ));
}

#[test]
fn stars_count_mismatch_test() {
    let mut bytes = sample_bytes();
    bytes[12] = 3;
    assert_eq!(
        stars_error(read(&bytes)),
        StarsError::CountMismatch {
            header: 3,
            items: 2
        }
    );
}

#[test]
fn stars_truncated_test() {
    let bytes = sample_bytes();
    // Cut inside the last chunk, with and without fixing the form size.
    let cut = &bytes[..bytes.len() - 1];
    assert_eq!(stars_error(read(cut)), StarsError::Truncated);
    let mut fixed = cut.to_vec();
    let size = u32::from_be_bytes(fixed[4..8].try_into().unwrap()) - 1;
    fixed[4..8].copy_from_slice(&size.to_be_bytes());
    assert_eq!(stars_error(read(&fixed)), StarsError::Truncated);
    // No count.
    assert_eq!(
        stars_error(read(b"FORM\0\0\0\x04STAR")),
        StarsError::Truncated
    );
}

#[test]
fn stars_invalid_item_test() {
    let good = item(SIRIUS, [0; 4], 1, b"A\0");
    // Name length past the chunk, short of it, and zero.
    for len in [3u32, 1, 0] {
        let mut data = good.clone();
        data[32..36].copy_from_slice(&len.to_le_bytes());
        assert_eq!(
            stars_error(read(&form(1, &[(b"ITEM", data)]))),
            StarsError::InvalidItem(0)
        );
    }
    // Shorter than the fixed fields.
    assert_eq!(
        stars_error(read(&form(1, &[(b"ITEM", good[..30].to_vec())]))),
        StarsError::InvalidItem(0)
    );
    // The index counts only stars.
    assert_eq!(
        stars_error(read(&form(
            2,
            &[(b"ITEM", good.clone()), (b"ITEM", good[..35].to_vec())]
        ))),
        StarsError::InvalidItem(1)
    );
}

#[test]
fn stars_invalid_name_test() {
    for name in [&b"AB"[..], b"A\0\0", b"\xFF\0"] {
        let data = item(SIRIUS, [0; 4], 1, name);
        assert_eq!(
            stars_error(read(&form(1, &[(b"ITEM", data)]))),
            StarsError::InvalidName(0),
            "{name:?}"
        );
    }

    let mut catalog = StarCatalog {
        stars: vec![sirius(), sirius()],
        ..Default::default()
    };
    catalog.stars[1].name = "A\0B".into();
    assert_eq!(
        stars_error(catalog.write(&mut Vec::new())),
        StarsError::InvalidName(1)
    );
}

const GAME_FILE: &str = "./tests/enfusion/test-data/stars_in/star_database.stars";

/// `system/star_database.stars`, the game's only catalog.
#[test]
fn stars_game_test() {
    let bytes = std::fs::read(GAME_FILE).unwrap();
    let catalog = read(&bytes).unwrap();
    assert_eq!(write(&catalog), bytes);

    assert_eq!(catalog.stars.len(), 128_503);
    assert!(catalog.unknown_chunks.is_empty());
    assert_eq!(catalog.stars[0], sirius());
    assert!(
        catalog
            .stars
            .windows(2)
            .all(|w| w[0].magnitude <= w[1].magnitude)
    );
    assert!(catalog.stars.iter().all(|s| s.rgba()[3] == 255));
    assert!(catalog.stars.iter().all(|s| {
        (0.0..24.0).contains(&s.right_ascension) && (-90.0..=90.0).contains(&s.declination)
    }));
    assert_eq!(
        catalog.stars.iter().filter(|s| !s.name.is_empty()).count(),
        162
    );

    let mut hips: Vec<u32> = catalog.stars.iter().map(|s| s.hip).collect();
    hips.sort_unstable();
    hips.dedup();
    assert_eq!(hips.len(), 117_942);

    // Three entries for Capella, the same star.
    let capella: Vec<&Star> = catalog.stars.iter().filter(|s| s.hip == 24608).collect();
    let names: Vec<&str> = capella.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["CAPELLA", "CAPELLA B", "CAPELLA A"]);
    assert!(
        capella
            .iter()
            .all(|s| s.declination == capella[0].declination)
    );

    let vega = catalog.find("Vega").unwrap();
    assert_eq!(vega.hip, 91262);
    assert!((vega.right_ascension - 18.615_65).abs() < 1e-4);
    assert!((vega.declination - 38.783_69).abs() < 1e-4);
    assert_eq!(vega.magnitude, 0.03);
}
