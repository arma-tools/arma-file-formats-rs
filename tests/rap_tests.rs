use std::{
    fs::{self, File},
    io::BufReader,
};

use rvff::rap::{Cfg, CfgEntry, CfgValue, EntryReturn};

const INPUT_PATH_PREFIX: &str = "./tests/test-data/rap_in/";
#[allow(dead_code)]
const OUTPUT_PATH_PREFIX: &str = "./tests/test-data/rap_out/";

#[test]
fn rvmat_define() {
    let file = File::open(format!("{}define.rvmat", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);
    if let Err(err) = Cfg::read(&mut reader) {
        println!("{}", err);
    }
}

#[test]
fn weird_roadlib() {
    let file = File::open(format!("{}roadslib_weird.cfg", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);
    if let Err(err) = Cfg::read(&mut reader) {
        println!("{}", err);
    }
}

#[test]
fn lzss_roadlib() {
    let file = File::open(format!("{}roadslib_lzss.cfg", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);
    if let Err(err) = Cfg::read(&mut reader) {
        println!("{}", err);
    }
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

    assert!(matches!(cfg.get_entry(&["comment"]), None));

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

    assert!(matches!(cfg.get_entry(&["comment"]), None));

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
