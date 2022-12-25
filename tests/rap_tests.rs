use std::{
    fs::{self, File},
    io::BufReader,
};

use rvff::rap::{Cfg, CfgEntry, CfgValue, EntryReturn};

const INPUT_PATH_PREFIX: &str = "./tests/test-data/rap_in/";
#[allow(dead_code)]
const OUTPUT_PATH_PREFIX: &str = "./tests/test-data/rap_out/";

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
