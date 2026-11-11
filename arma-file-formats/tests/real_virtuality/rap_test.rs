use std::{
    fs::{self, File},
    io::BufReader,
};

use arma_file_formats::real_virtuality::rap::{
    Cfg, CfgEntry, CfgEnumValue, CfgValue, EntryReturn, FsIncludeResolver, Preprocessor,
};

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/rap_in/";
#[allow(dead_code)]
const OUTPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/rap_out/";

#[test]
fn rvmat_define() {
    let file = File::open(format!("{}define.rvmat", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);
    let cfg = Cfg::read(&mut reader).unwrap();
    assert_eq!(
        cfg.get_entry(&["ambient"])
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn weird_roadlib() {
    let file = File::open(format!("{}roadslib_weird.cfg", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);
    let cfg = Cfg::read(&mut reader).unwrap();
    let width = cfg.get_entry(&["RoadTypesLibrary", "Road0001", "width"]);
    assert_eq!(width.unwrap().as_long(), Some(10));
}

#[test]
fn lzss_roadlib() {
    let file = File::open(format!("{}roadslib_lzss.cfg", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);
    let cfg = Cfg::read(&mut reader).unwrap();
    assert!(!cfg.entries.is_empty());
}

#[test]
fn read_config_needs_a_header() {
    let mut reader = std::io::Cursor::new(b"class A {};".to_vec());
    assert!(matches!(
        Cfg::read_config(&mut reader),
        Err(arma_file_formats::errors::AffError::InvalidFileError)
    ));
    // Without an enum table
    let mut data = b"\0raP".to_vec();
    data.extend([0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let cfg = Cfg::read_data(&data).unwrap();
    assert!(cfg.entries.is_empty() && cfg.enums.is_empty());
}

#[test]
fn lzss_config() {
    let file = File::open(format!("{}bmg_texture1.rvmat", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);
    let _ = Cfg::read(&mut reader).unwrap();
}

#[test]
fn mission_sqm_test() {
    let file_content = fs::read_to_string(format!("{}mission.sqm.cpp", INPUT_PATH_PREFIX)).unwrap();
    let cfg = Cfg::parse_config(&file_content).unwrap();

    // cfg.pretty_print(0);
    assert!(matches!(
        cfg.get_entry(&[
            "Mission",
            "Entities",
            "Item0",
            "Entities",
            "Item0",
            "Attributes",
            "isPlayer",
        ])
        .unwrap(),
        EntryReturn::Entry(CfgEntry::Property(x)) if &x.name == "isPlayer" && matches!(x.value, CfgValue::Long(num) if num == 1)
    ));

    assert!(cfg.get_entry(&["comment"]).is_none());

    assert!(matches!(cfg
        .get_entry(&["Mission", "Entities", "Item0", "Attributes", "FogD"])
        .unwrap(), EntryReturn::Entry(CfgEntry::Delete(x)) if &x == "FogD"));

    assert!(matches!(cfg
            .get_entry(&["FogE"])
            .unwrap(), EntryReturn::Entry(CfgEntry::Extern(x)) if &x == "FogE"));

    let cl = cfg.get_entry(&["AddonsMetaData"]).unwrap();
    assert!(
        matches!(cl, EntryReturn::Entry(CfgEntry::Class(class)) if class.name == "AddonsMetaData" &&
                class.parent.clone().unwrap() == "FogE" &&
                class.entries.len() == 1)
    );
}

#[test]
fn mission_sqm_bin_test() {
    let file = File::open(format!("{}mission.sqm.bin", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);
    let cfg = Cfg::read_config(&mut reader).unwrap();

    // cfg.pretty_print(0);

    assert!(matches!(
        cfg.get_entry(&[
            "Mission",
            "Entities",
            "Item0",
            "Entities",
            "Item0",
            "Attributes",
            "isPlayer",
        ])
        .unwrap(),
        EntryReturn::Entry(CfgEntry::Property(x)) if &x.name == "isPlayer" && matches!(x.value, CfgValue::Long(num) if num == 1)
    ));

    assert!(cfg.get_entry(&["comment"]).is_none());

    assert!(matches!(cfg
        .get_entry(&["Mission", "Entities", "Item0", "Attributes", "FogD"])
        .unwrap(), EntryReturn::Entry(CfgEntry::Delete(x)) if &x == "FogD"));

    assert!(matches!(cfg
            .get_entry(&["FogE"])
            .unwrap(), EntryReturn::Entry(CfgEntry::Extern(x)) if &x == "FogE"));

    let cl = cfg.get_entry(&["AddonsMetaData"]).unwrap();
    assert!(
        matches!(cl, EntryReturn::Entry(CfgEntry::Class(class)) if class.name == "AddonsMetaData" &&
                class.parent.clone().unwrap() == "FogE" &&
                class.entries.len() == 1)
    );
}

#[test]
fn comment_string_test() {
    let inp = "
    test = \"call{this addAction [\"\"<t color=\'#008000\'>Turn on Lights</t>\"\", \"\"Scripts\\XiviD\\lightsON.sqf\"\"];\" \\n \"this addAction [\"\"<t color='#FF0000'>Turn off Lights</t>\"\", \"\"Scripts\\XiviD\\lightsOFF.sqf\"\"];\" \\n \"\" \\n \"}\";

test2 = \"diag_log \"\"hi\"\"; \";

test3 = \"/*0*/\";
test4 = \"\";
/*
/*test5 = \"dasd\";
test5 = \"dasd\";

test5 = \"dasd\";
*

//23


fjaslkf*/
";

    let cfg = Cfg::parse_config(inp).unwrap();

    let test2 = cfg.get_entry(&["test"]).unwrap().as_string().unwrap();
    println!("test2: {}", test2);
    //fs::write("out_test.txt", test2).unwrap();
}

#[test]
fn grad_base_parse() {
    let file = File::open(format!("{}mission_grad_base.sqm", INPUT_PATH_PREFIX)).unwrap();
    let mut buf = BufReader::new(file);

    let cfg = Cfg::read(&mut buf).unwrap();
    //dbg!(cfg);

    let entry = cfg
        .get_entry(&["Mission", "Entities", "Item1066", "type"])
        .unwrap();

    assert_eq!(
        entry.as_string().unwrap_or_default(),
        "Land_Shoot_House_Panels_F"
    );
}

#[test]
fn entry_return_test() {
    let file = File::open(format!("{}test.rvmat", INPUT_PATH_PREFIX)).unwrap();
    let mut buf = BufReader::new(file);

    let cfg = Cfg::read(&mut buf).unwrap();

    let entry = cfg.get_entry(&["TexGen3", "uvTransform", "pos"]).unwrap();
    let n = entry.as_array().unwrap();
    dbg!(n);
}

#[test]
fn preprocessed_config() {
    let src = "#define QUOTE(x) #x\n#define VAL(x) value_##x = x;\n\
               class Cfg {\n    VAL(1)\n    name = QUOTE(abc);\n#ifdef MISSING\n    bad = 1;\n#endif\n};";
    let cfg = Cfg::parse_config(src).unwrap();
    assert_eq!(
        cfg.get_entry(&["Cfg", "value_1"]).unwrap().as_long(),
        Some(1)
    );
    assert_eq!(
        cfg.get_entry(&["Cfg", "name"])
            .unwrap()
            .as_string()
            .as_deref(),
        Some("abc")
    );
    assert!(cfg.get_entry(&["Cfg", "bad"]).is_none());

    assert!(Cfg::parse_config("#include \"x.hpp\"").is_err());
}

#[test]
fn preprocessed_config_includes() {
    let root = std::env::temp_dir().join(format!("aff-rap-include-{}", std::process::id()));
    let addon = root.join("x/mod/addons/main");
    fs::create_dir_all(&addon).unwrap();
    fs::write(
        addon.join("config.cpp"),
        "#include \"script_component.hpp\"\nclass CfgPatches { class ADDON { units[] = {}; }; };",
    )
    .unwrap();
    fs::write(
        addon.join("script_component.hpp"),
        "#include \"\\x\\mod\\addons\\main\\macros.hpp\"\n#define ADDON DOUBLES(mod,main)",
    )
    .unwrap();
    fs::write(addon.join("macros.hpp"), "#define DOUBLES(a,b) a##_##b").unwrap();

    let mut pp =
        Preprocessor::new().with_resolver(FsIncludeResolver::new().with_include_dir(&root));
    let path = addon.join("config.cpp");
    let cfg = Cfg::parse_path(path.to_str().unwrap(), &mut pp);
    fs::remove_dir_all(&root).unwrap();

    let cfg = cfg.unwrap();
    assert!(
        cfg.get_entry(&["CfgPatches", "mod_main", "units"])
            .is_some()
    );
}

#[test]
fn binary_append_int64_and_long_arrays() {
    let mut data = b"\0raP".to_vec();
    data.extend([0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0]);
    data.extend([0, 3]); // root: no parent, 3 entries
    // items[] += {"a"};
    data.push(5);
    data.extend(1_u32.to_le_bytes());
    data.extend(b"items\0");
    data.extend([1, 0, b'a', 0]);
    // big = 2^40;
    data.extend([1, 6]);
    data.extend(b"big\0");
    data.extend((1_i64 << 40).to_le_bytes());
    // many[] = {0, ..., 199}; the count takes two bytes
    data.push(2);
    data.extend(b"many\0");
    data.extend([0xC8, 0x01]);
    for i in 0..200_i32 {
        data.push(2);
        data.extend(i.to_le_bytes());
    }
    // enum { destructno = 0, destructman = 5 };
    let enum_offset = u32::try_from(data.len()).unwrap();
    data[12..16].copy_from_slice(&enum_offset.to_le_bytes());
    data.extend(2_u32.to_le_bytes());
    data.extend(b"destructno\0");
    data.extend(0_i32.to_le_bytes());
    data.extend(b"destructman\0");
    data.extend(5_i32.to_le_bytes());

    let cfg = Cfg::read_data(&data).unwrap();
    assert_eq!(
        cfg.enums,
        [
            CfgEnumValue {
                name: "destructno".to_owned(),
                value: 0
            },
            CfgEnumValue {
                name: "destructman".to_owned(),
                value: 5
            }
        ]
    );
    let CfgEntry::Property(items) = &cfg.entries[0] else {
        panic!("{:?}", cfg.entries[0]);
    };
    assert!(items.append);
    assert_eq!(
        items.value,
        CfgValue::Array(vec![CfgValue::String("a".to_owned())])
    );
    assert_eq!(
        cfg.get_entry(&["big"]).unwrap().as_value(),
        Some(CfgValue::Int64(1 << 40))
    );
    let many = cfg.get_entry(&["many"]).unwrap().as_array().unwrap();
    assert_eq!(many.len(), 200);
    assert_eq!(many[199], CfgValue::Long(199));

    // Unknown types are errors
    let mut bad = data[..18].to_vec();
    bad[17] = 1;
    bad.push(9);
    assert!(Cfg::read_data(&bad).is_err());
}

#[test]
fn text_values_like_binarized() {
    let cfg = Cfg::parse_config(
        "a = true; b = 1.0; c = 0.2*4/3; d = 007; e = 0x10; f = $STR_x;\n\
         class 30Rnd_mag; class B: 30Rnd_mag { items[] += {x, 1}; };",
    )
    .unwrap();
    let value = |name| cfg.get_entry(&[name]).unwrap().as_value().unwrap();
    assert_eq!(value("a"), CfgValue::String("true".to_owned()));
    assert_eq!(value("b"), CfgValue::Float(1.0));
    assert_eq!(value("c"), CfgValue::String("0.2*4/3".to_owned()));
    assert_eq!(value("d"), CfgValue::Long(7));
    assert_eq!(value("e"), CfgValue::Long(16));
    assert_eq!(value("f"), CfgValue::String("$STR_x".to_owned()));
    let EntryReturn::Entry(CfgEntry::Property(items)) = cfg.get_entry(&["B", "items"]).unwrap()
    else {
        panic!();
    };
    assert!(items.append);
}

#[test]
fn entry_accessors() {
    let cfg = Cfg::parse_config(
        "class Ext; delete Del; f = 1.5; l = 2; s = \"x\"; a[] = {1};\n\
         class C: Ext { class Ext2; delete Del2; inner = 3; i64 = 4294967296; class D {}; };",
    )
    .unwrap();
    let entry = |path: &[&str]| cfg.get_entry(path).unwrap();

    // Top level properties are values, the others entries
    assert_eq!(entry(&["f"]).as_float(), Some(1.5));
    assert_eq!(entry(&["l"]).as_long(), Some(2));
    assert_eq!(entry(&["s"]).as_string().as_deref(), Some("x"));
    assert_eq!(entry(&["a"]).as_array(), Some(vec![CfgValue::Long(1)]));
    assert!(entry(&["f"]).as_entry().is_none());
    assert_eq!(entry(&["C", "inner"]).as_long(), Some(3));
    assert!(entry(&["C", "inner"]).as_value().is_none());
    assert_eq!(
        entry(&["C", "inner"]).as_property().map(|p| p.name),
        Some("inner".to_owned())
    );
    assert_eq!(
        entry(&["C", "i64"])
            .as_value()
            .or_else(|| entry(&["C", "i64"]).as_property().map(|p| p.value)),
        Some(CfgValue::Int64(4_294_967_296))
    );
    assert_eq!(
        entry(&["C", "i64"]).as_property().unwrap().value.as_int64(),
        Some(4_294_967_296)
    );
    assert_eq!(entry(&["l"]).as_value().unwrap().as_int64(), Some(2));
    assert_eq!(entry(&["Ext"]).as_extern().as_deref(), Some("Ext"));
    assert_eq!(entry(&["Del"]).as_delete().as_deref(), Some("Del"));
    assert_eq!(entry(&["C", "Ext2"]).as_extern().as_deref(), Some("Ext2"));
    assert_eq!(entry(&["C", "Del2"]).as_delete().as_deref(), Some("Del2"));
    assert_eq!(
        entry(&["C", "D"]).as_class().map(|c| c.name),
        Some("D".to_owned())
    );
    assert_eq!(
        entry(&["C"]).as_class().unwrap().parent.as_deref(),
        Some("Ext")
    );

    // Wrong kinds
    let ext = entry(&["Ext"]);
    assert!(ext.as_class().is_none() && ext.as_property().is_none() && ext.as_delete().is_none());
    assert!(ext.as_float().is_none() && ext.as_long().is_none());
    assert!(ext.as_string().is_none() && ext.as_array().is_none());
    assert!(entry(&["Del"]).as_extern().is_none());
    let value = entry(&["s"]).as_value().unwrap();
    assert!(value.as_float().is_none() && value.as_long().is_none() && value.as_int64().is_none());
    assert!(value.as_array().is_none());
    assert!(CfgValue::Long(1).as_string().is_none());

    // Missing
    for path in [
        &["nope"][..],
        &["C", "nope"],
        &["Ext", "x"],
        &["f", "x"],
        &[],
    ] {
        assert!(cfg.get_entry(path).is_none(), "{path:?}");
    }
}

#[test]
fn binary_class_loops_fail() {
    // class A { class A ... } pointing at its own body
    let mut data = b"\0raP".to_vec();
    data.extend([0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0]);
    data.extend([0, 1, 0, b'A', 0]);
    let body = u32::try_from(data.len() + 4).unwrap();
    data.extend(body.to_le_bytes());
    data.extend([0, 1, 0, b'A', 0]);
    data.extend(body.to_le_bytes());
    let err = Cfg::read_data(&data).unwrap_err();
    assert!(err.to_string().contains("nested deeper"), "{err}");
}

#[test]
fn test_data_round_trips() {
    let mission_bin = fs::read(format!("{INPUT_PATH_PREFIX}mission.sqm.bin")).unwrap();
    let texts = [
        "mission.sqm.cpp",
        "test.rvmat",
        "mission_grad_base.sqm",
        "roadslib_weird.cfg",
    ];
    let mut configs = vec![Cfg::read_data(&mission_bin).unwrap()];
    for name in texts {
        let text = fs::read_to_string(format!("{INPUT_PATH_PREFIX}{name}")).unwrap();
        configs.push(Cfg::parse_config(&text).unwrap());
    }
    for cfg in configs {
        let again = Cfg::parse_config(&cfg.to_string()).unwrap();
        assert_eq!(again.entries, cfg.entries);
        assert_eq!(again.enums, cfg.enums);
    }
}
