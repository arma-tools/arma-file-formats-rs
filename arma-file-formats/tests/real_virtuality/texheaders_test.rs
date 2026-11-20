//! Tests for the `texHeaders.bin` reader, writer and builder.
//!
//! `test-data/texheaders_in/` holds Arma 3 files: `editor_f/` the `texHeaders.bin` of
//! `editor_f.pbo` and its only texture, `3den_texHeaders.bin` (303 textures), and in `textures/`
//! 21 textures of several PBOs (all pixel formats but ARGB1555, `.pac` files, every texture
//! type, alpha flags) with a `texHeaders.bin` of their headers taken from the game's files.

use std::io::Cursor;

use arma_file_formats::{
    errors::{AffError, TexHeadersError},
    real_virtuality::texheaders::{
        DEFAULT_MAX_COLOR, PixelFormat, TexHeader, TexHeaderMipmap, TexHeaders, TextureType,
    },
};

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/texheaders_in/";

fn input(name: &str) -> Vec<u8> {
    std::fs::read(format!("{INPUT_PATH_PREFIX}{name}")).unwrap()
}

fn read(bytes: &[u8]) -> Result<TexHeaders, AffError> {
    TexHeaders::from(&mut Cursor::new(bytes))
}

fn write(headers: &TexHeaders) -> Vec<u8> {
    let mut out = Vec::new();
    headers.write(&mut out).unwrap();
    out
}

fn error(result: Result<impl std::fmt::Debug, AffError>) -> TexHeadersError {
    match result {
        Err(AffError::TexHeadersError(e)) => e,
        other => panic!("expected a TexHeadersError, got {other:?}"),
    }
}

/// A color byte as the header stores it.
fn c(byte: u8) -> f32 {
    f32::from(byte) * (1.0 / 255.0)
}

/// A DXT5 `.paa` with these tags and one 4x4 mipmap.
fn paa(tags: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
    let mut paa = vec![0x05, 0xff];
    for (name, data) in tags {
        paa.extend(b"GGAT");
        paa.extend(*name);
        paa.extend((data.len() as u32).to_le_bytes());
        paa.extend(*data);
    }
    paa.extend([0, 0]);
    paa.extend([4, 0, 4, 0, 16, 0, 0]);
    paa.extend([0; 16]);
    paa.extend([0, 0, 0, 0]);
    paa
}

#[test]
fn texheaders_round_trip_test() {
    for name in [
        "editor_f/texHeaders.bin",
        "3den_texHeaders.bin",
        "textures/texHeaders.bin",
    ] {
        let bytes = input(name);
        assert_eq!(write(&read(&bytes).unwrap()), bytes, "{name}");
    }
}

#[test]
fn texheaders_fields_test() {
    let headers = read(&input("editor_f/texHeaders.bin")).unwrap();
    assert_eq!(headers.version, 1);
    assert_eq!(headers.textures.len(), 1);
    let t = &headers.textures[0];
    assert_eq!(t.path, "data\\object_composition_ca.paa");
    assert_eq!(t.pixel_format, PixelFormat::Dxt5);
    assert_eq!(t.texture_type, TextureType::Diffuse);
    assert_eq!(t.average_color, [c(253), c(253), c(253), c(57)]);
    // Times 1 / 255, not divided by 255.
    assert_eq!(t.average_color[0], 0.992_156_9);
    assert_ne!(t.average_color[0], 253.0 / 255.0);
    assert_eq!(t.max_color, DEFAULT_MAX_COLOR);
    assert!(t.has_max_color && t.is_alpha && !t.is_transparent && t.is_alpha_non_opaque);
    assert!(t.is_paa && t.little_endian);
    assert_eq!(t.file_size, 25272);
    assert_eq!(t.mipmaps.len(), 7);
    assert_eq!(
        t.mipmaps[0],
        TexHeaderMipmap {
            width: 256,
            height: 256,
            reserved: 0,
            pixel_format: PixelFormat::Dxt5,
            unknown: 3,
            offset: 128,
        }
    );
    assert_eq!(t.mipmaps[6].width, 4);
}

#[test]
fn texheaders_build_like_binarize_test() {
    let header = TexHeader::from_paa(
        "Data\\object_composition_ca.paa",
        &input("editor_f/Data/object_composition_ca.paa"),
    )
    .unwrap();
    let headers = TexHeaders::from_textures(vec![header]);
    assert_eq!(write(&headers), input("editor_f/texHeaders.bin"));
}

#[test]
fn texheaders_from_paa_test() {
    let expected = read(&input("textures/texHeaders.bin")).unwrap();
    assert_eq!(expected.textures.len(), 21);
    for texture in &expected.textures {
        let file = input(&format!("textures/{}", texture.path.replace('\\', "/")));
        let header = TexHeader::from_paa(&texture.path, &file).unwrap();
        assert_eq!(&header, texture, "{}", texture.path);
    }
}

#[test]
fn texheaders_order_test() {
    // 303 textures, so the table grows from 15 to 31 buckets.
    let game = read(&input("3den_texHeaders.bin")).unwrap();
    let mut shuffled = game.clone();
    shuffled.textures.reverse();
    shuffled.textures.rotate_left(100);
    shuffled.sort_like_binarize();
    assert_eq!(shuffled, game);
}

#[test]
fn texheaders_texture_type_test() {
    for (path, texture_type) in [
        ("a_co.paa", TextureType::Diffuse),
        ("a_ca.paa", TextureType::Diffuse),
        ("a.paa", TextureType::Diffuse),
        ("a_lco.paa", TextureType::DiffuseLinear),
        ("sky_a_lco.paa", TextureType::DiffuseLinear),
        ("a_sky.paa", TextureType::DiffuseLinear),
        ("a_dt.paa", TextureType::Detail),
        ("a_mco.paa", TextureType::Detail),
        ("a_nohq.paa", TextureType::Normal),
        ("a_normalmap.paa", TextureType::Normal),
        ("a_mc.paa", TextureType::Macro),
        ("a_as.paa", TextureType::AmbientShadow),
        ("a_smdi.paa", TextureType::Specular),
        ("a_dtsmdi.paa", TextureType::DetailSpecular),
        ("a_mask.paa", TextureType::Mask),
        ("a_ti_ca.paa", TextureType::Thermal),
        ("A_TI_CA.PAA", TextureType::Thermal),
        // From the last `_`, up to a `.`.
        ("a_nohq.x.paa", TextureType::Normal),
        ("a_ti_ca.x.paa", TextureType::Thermal),
        ("x_mask.y_dt.paa", TextureType::Detail),
        ("a_nohq_ca.paa", TextureType::Diffuse),
        ("a_ti_x_ca.paa", TextureType::Diffuse),
        ("ti_ca.paa", TextureType::Diffuse),
        ("a_tiger_ca.paa", TextureType::Diffuse),
        ("a_mc_as.paa", TextureType::AmbientShadow),
        // Only the file name counts.
        ("x_dt.dir\\a.paa", TextureType::Diffuse),
        ("sub\\a_mask.paa", TextureType::Mask),
        // Satellite tiles.
        ("s_004_024_lco.paa", TextureType::Diffuse),
        ("s_abcdefg_lco.paa", TextureType::Diffuse),
        ("s_abcdefg_lco.x.paa", TextureType::Diffuse),
        ("sub\\s_004_024_lco.pac", TextureType::Diffuse),
        ("s_04_24_lco.paa", TextureType::DiffuseLinear),
        ("s_1234_5678_lco.paa", TextureType::DiffuseLinear),
        ("xs_004_024_lco.paa", TextureType::DiffuseLinear),
        ("s_004_024_sky.paa", TextureType::DiffuseLinear),
        ("s_004_024_mask.paa", TextureType::Mask),
    ] {
        assert_eq!(TextureType::from_path(path), texture_type, "{path}");
    }
    for value in 0..16 {
        assert_eq!(TextureType::from_u32(value).to_u32(), value);
    }
}

#[test]
fn texheaders_from_paa_tags_test() {
    // Without tags: the default average color, no max color, no alpha.
    let t = TexHeader::from_paa("Data/A_CO.paa", &paa(&[])).unwrap();
    assert_eq!(t.path, "data\\a_co.paa");
    assert_eq!(t.average_color, [c(128), c(32), c(32), 1.0]);
    assert_eq!(t.max_color, DEFAULT_MAX_COLOR);
    assert!(!t.has_max_color && !t.is_alpha && !t.is_transparent && !t.is_alpha_non_opaque);
    assert_eq!(t.mipmaps.len(), 1);
    assert_eq!(t.mipmaps[0].offset, 4);
    assert_eq!(t.file_size, 31);

    // AVGC is BGRA, MAXC is read as u32.
    let t = TexHeader::from_paa(
        "a.paa",
        &paa(&[
            (b"CGVA", &[10, 20, 30, 40]),
            (b"CXAM", &[0x11, 0x22, 0x33, 0x44]),
        ]),
    )
    .unwrap();
    assert_eq!(t.average_color, [c(30), c(20), c(10), c(40)]);
    assert_eq!(t.max_color, 0x4433_2211);
    assert!(t.has_max_color);

    // Alpha below 128 with interpolated alpha.
    for (flag, alpha, alpha_flags) in [
        (1, 127, (true, false, true)),
        (1, 128, (true, false, false)),
        (2, 0, (false, true, false)),
        (3, 16, (true, true, true)),
        (4, 0, (false, false, false)),
    ] {
        let t = TexHeader::from_paa(
            "a.paa",
            &paa(&[(b"CGVA", &[0, 0, 0, alpha]), (b"GALF", &[flag, 0, 0, 0])]),
        )
        .unwrap();
        assert_eq!(
            (t.is_alpha, t.is_transparent, t.is_alpha_non_opaque),
            alpha_flags,
            "{flag} {alpha}"
        );
    }

    // Other tags are skipped, `.pac` files aren't PAA.
    let t = TexHeader::from_paa("a.PAC", &paa(&[(b"SFFO", &[0; 64])])).unwrap();
    assert!(!t.is_paa);
}

#[test]
fn texheaders_errors_test() {
    assert_eq!(
        error(read(b"OPRW\x01\0\0\0\0\0\0\0")),
        TexHeadersError::UnknownSignature("OPRW".to_string())
    );
    assert_eq!(
        error(read(b"0DHT\x02\0\0\0\0\0\0\0")),
        TexHeadersError::UnsupportedVersion(2)
    );
    let bytes = input("editor_f/texHeaders.bin");
    assert_eq!(
        error(read(&bytes[..bytes.len() - 1])),
        TexHeadersError::Truncated
    );
    let mut longer = bytes.clone();
    longer.push(0);
    assert_eq!(error(read(&longer)), TexHeadersError::TrailingData(1));

    let mut unknown_type = paa(&[]);
    unknown_type[0..2].copy_from_slice(&0x1234u16.to_le_bytes());
    assert_eq!(
        TexHeader::from_paa("a.paa", &unknown_type),
        Err(TexHeadersError::UnknownPaaType(0x1234))
    );
    // Binarize skips textures with such tags.
    assert_eq!(
        TexHeader::from_paa("a.paa", &paa(&[(b"CGVA", &[1, 2, 3])])),
        Err(TexHeadersError::InvalidTag("CGVA".to_string(), 3))
    );
    let truncated = paa(&[]);
    assert_eq!(
        TexHeader::from_paa("a.paa", &truncated[..truncated.len() - 3]),
        Err(TexHeadersError::Truncated)
    );
}
