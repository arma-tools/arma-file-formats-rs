//! Tests for the Enfusion text format, `.emat` materials and resource references.
//!
//! The snippets follow the syntax found in Arma Reforger's materials, including its quirks
//! (CRLF line endings, a missing closing brace, the id before the parent).

use std::collections::HashMap;

use arma_file_formats::{
    enfusion::{
        emat::{Emat, UvTransform},
        resource::ResourceRef,
        text::{Object, Value},
    },
    errors::{AffError, EmatError, TextError},
};

const MULTI: &str = r#"MatPBRMulti {
 UVSrcGlobalMaps "UV set 2"
 MaskMap "{B8F9ACECAB535A89}Assets/Wheel/Data/Tire_GLOBAL_MASK.edds"
 Color_1 0.039 0.039 0.039 1
 BCR_1 "{B141E00CC94C086A}Assets/_SharedData/Rubber/ST_Rubber_BCR.edds"
 UVTransform_1 MatUVTransform "{51823C7CB6F81D33}" {
  TilingU 4
  TilingV 2
 }
 "Wind power" 2
 CausticMap "{69750C9CCD3FE588}Water/caustics_00.edds" "{9D7D4B04C0E28DFC}Water/caustics_01.edds" anim( 20 )
 BCRMap "{283AEF6863A887E5}system/textures/roadEndDefault.edds" clampu clampv
 Refs {
  "Enabled"
  "SCR_DepthOfFieldEffect.s_bEnableDOF"
 }
 UserUVTexMat {
  1 0 0 0
 }
}
"#;

#[test]
fn parses_material_properties() {
    let material = Emat::parse(MULTI).unwrap();
    assert_eq!(material.class, "MatPBRMulti");
    assert_eq!(material.parent, None);

    let mask = material.texture("MaskMap").unwrap();
    assert_eq!(mask.resource().guid, Some(0xB8F9_ACEC_AB53_5A89));
    assert_eq!(
        mask.resource().path,
        "Assets/Wheel/Data/Tire_GLOBAL_MASK.edds"
    );
    assert_eq!(material.color("Color_1"), Some([0.039, 0.039, 0.039, 1.0]));
    assert_eq!(material.float("Wind power"), Some(2.0));
    assert_eq!(
        material.uv_transform("UVTransform_1"),
        Some(UvTransform {
            tiling: [4.0, 2.0],
            ..UvTransform::default()
        })
    );

    let caustics = material.texture("CausticMap").unwrap();
    assert_eq!(caustics.frames.len(), 2);
    assert_eq!(caustics.fps, Some(20.0));

    let road = material.texture("BCRMap").unwrap();
    assert!(road.has_flag("clampu") && road.has_flag("clampv") && !road.has_flag("nofilter"));

    let slots: Vec<_> = material.textures().map(|(key, _)| key).collect();
    assert_eq!(slots, ["MaskMap", "BCR_1", "CausticMap", "BCRMap"]);
    assert!(
        material.texture("UVSrcGlobalMaps").is_none(),
        "a string, but not a texture"
    );
}

#[test]
fn keeps_lists_and_matrices() {
    let material = Emat::parse(MULTI).unwrap();
    let refs = material.object.object("Refs").unwrap();
    assert_eq!(refs.class, None);
    let keys: Vec<_> = refs.entries.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys, ["Enabled", "SCR_DepthOfFieldEffect.s_bEnableDOF"]);

    let row = &material.object.object("UserUVTexMat").unwrap().entries[0];
    assert_eq!(row.key, "1");
    assert_eq!(
        row.values,
        [
            Value::Word("0".into()),
            Value::Word("0".into()),
            Value::Word("0".into())
        ]
    );

    let uv = material.object.object("UVTransform_1").unwrap();
    assert_eq!(uv.class.as_deref(), Some("MatUVTransform"));
    assert_eq!(uv.id.as_deref(), Some("{51823C7CB6F81D33}"));
}

#[test]
fn tolerates_quirks_of_game_files() {
    // CRLF line endings and no closing brace at the end of the file.
    let unclosed = "MatPBRBasic : \"{5CDDE2709BA9A5EE}Assets/_BaseMaterials/Base.emat\" {\r\n Color 1 1 1 1\r\n BCRMap \"{33F2B8BE5B29983C}Eyes_BCR.edds\" \r\n";
    let material = Emat::parse(unclosed).unwrap();
    assert_eq!(
        material.parent.as_ref().unwrap().path,
        "Assets/_BaseMaterials/Base.emat"
    );
    assert!(material.texture("BCRMap").is_some());

    // Id before the parent.
    let with_id =
        "MatPBRBasic \"{9D0D26B26B7BC3D8}Helmet.emat\" : \"{5CDDE2709BA9A5EE}Base.emat\" {\n}\n";
    let object = Object::parse(with_id).unwrap();
    assert_eq!(object.id.as_deref(), Some("{9D0D26B26B7BC3D8}Helmet.emat"));
    assert_eq!(
        object.parent.as_deref(),
        Some("{5CDDE2709BA9A5EE}Base.emat")
    );
}

#[test]
fn reports_the_line_of_errors() {
    let err = Object::parse("MatPBRBasic {\n Color 1 1\n BCRMap anim( 1 \n}\n").unwrap_err();
    assert!(
        matches!(err, AffError::TextError(TextError::Parse { line: 3, .. })),
        "{err}"
    );
    assert!(Object::parse("{ }").is_err());
    assert!(Object::parse("MatPBRBasic { }\nMatPBRBasic { }").is_err());
}

fn library() -> HashMap<String, Emat> {
    let files = [
        (
            "Base.emat",
            r#"MatPBRBasic {
 Color 0.5 0.5 0.5 1
 BCRMap "{0000000000000001}Base_BCR.edds"
 NMOMap "{0000000000000002}Base_NMO.edds"
 BCRMapUVTransform MatUVTransform "{1111111111111111}" {
  TilingU 4
  TilingV 2
 }
}"#,
        ),
        (
            "Mid.emat",
            r#"MatPBRBasic : "{00000000000000AA}base.EMAT" {
 Color 1 0 0 1
 BCRMapUVTransform MatUVTransform "{2222222222222222}" {
  TilingU 8
 }
}"#,
        ),
        (
            "LoopA.emat",
            r#"MatPBRBasic : "LoopB.emat" {
}"#,
        ),
        (
            "LoopB.emat",
            r#"MatPBRBasic : "LoopA.emat" {
}"#,
        ),
    ];
    files
        .into_iter()
        .map(|(path, text)| (path.to_lowercase(), Emat::parse(text).unwrap()))
        .collect()
}

#[test]
fn resolves_inheritance() {
    let library = library();
    let child = Emat::parse(
        r#"MatPBRBasic : "{00000000000000BB}Mid.emat" {
 NMOMap "{0000000000000003}Child_NMO.edds"
}"#,
    )
    .unwrap();
    let resolved = child
        .resolve(|parent| library.get(&parent.lookup_key()).cloned())
        .unwrap();

    assert_eq!(
        resolved.color("Color"),
        Some([1.0, 0.0, 0.0, 1.0]),
        "from Mid"
    );
    assert_eq!(
        resolved.texture("BCRMap").unwrap().resource().path,
        "Base_BCR.edds"
    );
    assert_eq!(
        resolved.texture("NMOMap").unwrap().resource().path,
        "Child_NMO.edds"
    );
    assert_eq!(
        resolved.uv_transform("BCRMapUVTransform").unwrap().tiling,
        [8.0, 2.0],
        "nested objects are merged field by field"
    );
    assert_eq!(resolved.parent.unwrap().path, "Mid.emat");
}

#[test]
fn inheritance_errors() {
    let library = library();
    let load = |parent: &ResourceRef| library.get(&parent.lookup_key()).cloned();

    let orphan = Emat::parse("MatPBRBasic : \"Missing.emat\" {\n}").unwrap();
    assert!(matches!(
        orphan.resolve(load),
        Err(AffError::EmatError(EmatError::ParentNotFound(path))) if path == "Missing.emat"
    ));

    let looping = Emat::parse("MatPBRBasic : \"LoopA.emat\" {\n}").unwrap();
    assert!(matches!(
        looping.resolve(load),
        Err(AffError::EmatError(EmatError::InheritanceCycle(_)))
    ));
}

#[test]
fn resource_references() {
    let full = ResourceRef::parse("{3C10D749A4E84B38}Common/Materials/Default.emat").unwrap();
    assert_eq!(full.guid, Some(0x3C10_D749_A4E8_4B38));
    assert_eq!(full.path, "Common/Materials/Default.emat");
    assert_eq!(
        full.to_string(),
        "{3C10D749A4E84B38}Common/Materials/Default.emat"
    );
    assert_eq!(full.lookup_key(), "common/materials/default.emat");

    let guid_only = ResourceRef::parse("{51823C7CB6F81D33}").unwrap();
    assert_eq!(
        (guid_only.guid, guid_only.path.as_str()),
        (Some(0x5182_3C7C_B6F8_1D33), "")
    );

    let path_only = ResourceRef::parse(r"Assets\Props\Box.xob").unwrap();
    assert_eq!(
        (path_only.guid, path_only.path.as_str()),
        (None, "Assets/Props/Box.xob")
    );

    assert_eq!(ResourceRef::parse(""), None);
    assert_eq!(ResourceRef::parse("{not hex}x.emat").unwrap().guid, None);
}
