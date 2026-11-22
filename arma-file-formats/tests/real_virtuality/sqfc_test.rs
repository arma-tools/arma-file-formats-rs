//! Tests for the `.sqfc` compiled script reader and writer.
//!
//! `test-data/sqfc_in/` holds Arma 3 scripts: `fn_spawnGroup.sqfc` and
//! `fn_ambientAnim__terminate.sqfc` (arrays and nular constants) of `functions_f.pbo`,
//! `fn_dbSymbolClass.sqfc` (no names, no name table) and `RscDisplayMovieInterrupt.sqfc` of
//! `ui_f.pbo` (code from an included file).

use std::io::Cursor;

use arma_file_formats::{
    errors::{AffError, SqfcError},
    real_virtuality::sqfc::{
        SQFC_VERSION_1, SQFC_VERSION_2, Sqfc, SqfcCode, SqfcCodeSource, SqfcConstant,
        SqfcInstruction, SqfcLocation,
    },
};

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/sqfc_in/";

const FILES: [&str; 4] = [
    "fn_spawnGroup.sqfc",
    "fn_ambientAnim__terminate.sqfc",
    "fn_dbSymbolClass.sqfc",
    "RscDisplayMovieInterrupt.sqfc",
];

fn input(name: &str) -> Vec<u8> {
    std::fs::read(format!("{INPUT_PATH_PREFIX}{name}")).unwrap()
}

fn read(bytes: &[u8]) -> Result<Sqfc, AffError> {
    Sqfc::from(&mut Cursor::new(bytes))
}

fn write(sqfc: &Sqfc) -> Vec<u8> {
    let mut out = Vec::new();
    sqfc.write(&mut out).unwrap();
    out
}

fn error(result: Result<impl std::fmt::Debug, AffError>) -> SqfcError {
    match result {
        Err(AffError::SqfcError(e)) => e,
        other => panic!("expected an SqfcError, got {other:?}"),
    }
}

fn write_error(sqfc: &Sqfc) -> SqfcError {
    error(sqfc.write(&mut Vec::new()))
}

fn code(constant: &SqfcConstant) -> &SqfcCode {
    match constant {
        SqfcConstant::Code(code) => code,
        other => panic!("expected code, got {other:?}"),
    }
}

const fn at(offset: u32, line: u16) -> SqfcLocation {
    SqfcLocation {
        offset,
        file: 0,
        line,
    }
}

fn get_variable(name: &str, offset: u32, line: u16) -> SqfcInstruction {
    SqfcInstruction::GetVariable {
        name: name.into(),
        location: at(offset, line),
    }
}

/// A script with one code block of `instructions`, the constants after it.
fn script(version: u32, instructions: Vec<SqfcInstruction>, constants: Vec<SqfcConstant>) -> Sqfc {
    let mut all = vec![SqfcConstant::Code(SqfcCode {
        source: SqfcCodeSource::Constant(1),
        instructions,
    })];
    all.push(SqfcConstant::String("source".into()));
    all.extend(constants);
    Sqfc {
        version,
        constants: all,
        files: vec!["a.sqf".into()],
        code_index: 0,
        name_table: Some(Vec::new()),
        compressed: true,
    }
}

/// The bytes of a string.
fn string(text: &str) -> Vec<u8> {
    let mut out = (text.len() as u32).to_le_bytes()[..3].to_vec();
    out.extend(text.as_bytes());
    out
}

/// Version 1, an uncompressed constants block with `constants` (count included), a code block.
fn raw(constants: &[u8]) -> Vec<u8> {
    let mut out = SQFC_VERSION_1.to_le_bytes().to_vec();
    out.push(0);
    out.extend(constants);
    out.push(3);
    out.extend(0u64.to_le_bytes());
    out
}

/// A compressed block of `data`.
fn compressed(block: u8, data: &[u8]) -> Vec<u8> {
    let mut out = vec![block];
    out.extend((data.len() as u32).to_le_bytes());
    out.push(2);
    out.extend(lzokay_native::compress(data).unwrap());
    out
}

#[test]
fn sqfc_round_trip_test() {
    for name in FILES {
        let bytes = input(name);
        assert_eq!(write(&read(&bytes).unwrap()), bytes, "{name}");
    }
}

#[test]
fn sqfc_spawn_group_test() {
    let sqfc = read(&input("fn_spawnGroup.sqfc")).unwrap();
    assert_eq!(sqfc.version, SQFC_VERSION_1);
    assert!(sqfc.compressed);
    assert_eq!(sqfc.constants.len(), 112);
    assert_eq!(sqfc.code_index, 111);
    assert_eq!(
        sqfc.files,
        ["\\A3\\Functions_F\\Spawning\\fn_spawnGroup.sqf"]
    );
    let source = sqfc.source().unwrap();
    assert!(source.starts_with("#line 0 \"\\A3\\Functions_F\\Spawning\\fn_spawnGroup.sqf\"\n"));
    let main = sqfc.code().unwrap();
    assert_eq!(main.source, SqfcCodeSource::Constant(110));
    assert_eq!(sqfc.code_text(main), Some(source));
    // `if (count _this < 3) exitWith {...}`, names lower case.
    assert_eq!(
        main.instructions[..8],
        [
            SqfcInstruction::EndStatement,
            get_variable("_this", 98, 35),
            SqfcInstruction::CallUnary {
                name: "count".into(),
                location: at(92, 35)
            },
            SqfcInstruction::Push { constant: 0 },
            SqfcInstruction::CallBinary {
                name: "<".into(),
                location: at(105, 35)
            },
            SqfcInstruction::CallUnary {
                name: "if".into(),
                location: at(87, 35)
            },
            SqfcInstruction::Push { constant: 3 },
            SqfcInstruction::CallBinary {
                name: "exitwith".into(),
                location: at(110, 35)
            },
        ]
    );
    assert_eq!(main.instructions[1].name(), Some("_this"));
    assert_eq!(main.instructions[0].name(), None);
    assert_eq!(main.instructions[0].location(), None);
    assert_eq!(
        sqfc.file(main.instructions[1].location().unwrap()),
        Some("\\A3\\Functions_F\\Spawning\\fn_spawnGroup.sqf")
    );

    // Arma 3's compiler sorts the table.
    let mut sorted = sqfc.clone();
    sorted.sort_name_table();
    assert_eq!(sorted, sqfc);
    assert!(sqfc.name_table.as_ref().unwrap().is_sorted());
}

#[test]
fn sqfc_constants_test() {
    let sqfc = read(&input("fn_ambientAnim__terminate.sqfc")).unwrap();
    assert_eq!(sqfc.constants.len(), 43);
    assert_eq!(sqfc.constants[0], SqfcConstant::String("_unit".into()));
    assert_eq!(sqfc.constants[5], SqfcConstant::Boolean(false));
    assert_eq!(
        sqfc.constants[8],
        SqfcConstant::NularCommand("objnull".into())
    );
    assert_eq!(sqfc.constants[17], SqfcConstant::Scalar(1.0));

    // `{ _x call BIS_fnc_ambientAnim__terminate; }` inside `{ {...} forEach _this; }`.
    let inner = code(&sqfc.constants[6]);
    assert_eq!(
        inner.source,
        SqfcCodeSource::Range {
            offset: 219,
            length: 44
        }
    );
    assert_eq!(
        sqfc.code_text(inner),
        Some("\n\t\t_x call BIS_fnc_ambientAnim__terminate;\n\t")
    );
    assert_eq!(
        inner.instructions,
        [
            SqfcInstruction::EndStatement,
            get_variable("_x", 222, 10),
            get_variable("bis_fnc_ambientanim__terminate", 230, 10),
            SqfcInstruction::CallBinary {
                name: "call".into(),
                location: at(225, 10)
            },
        ]
    );
    let outer = code(&sqfc.constants[7]);
    assert_eq!(outer.instructions[1], SqfcInstruction::Push { constant: 6 });
    // `{}`
    let empty = code(&sqfc.constants[9]);
    assert_eq!(sqfc.code_text(empty), Some(""));
    assert!(empty.instructions.is_empty());

    // An array of two code blocks.
    let SqfcConstant::Array { flags, items } = &sqfc.constants[40] else {
        panic!("expected an array");
    };
    assert_eq!(*flags, 0);
    assert_eq!(items.len(), 2);
    assert_eq!(
        sqfc.code_text(code(&items[0])),
        Some(
            "
	_unit call _detachCode;
"
        )
    );
    assert!(matches!(
        code(&items[1]).instructions[3],
        SqfcInstruction::MakeArray { size: 2, .. }
    ));

    let main = sqfc.code().unwrap();
    assert_eq!(
        main.instructions[6..8],
        [
            SqfcInstruction::MakeArray {
                size: 5,
                location: at(73, 3)
            },
            SqfcInstruction::CallUnary {
                name: "private".into(),
                location: at(66, 3)
            },
        ]
    );
    assert_eq!(
        main.instructions[9..11],
        [
            SqfcInstruction::Push { constant: 5 },
            SqfcInstruction::AssignTo {
                name: "_fnc_log_disable".into(),
                location: at(161, 5)
            },
        ]
    );
}

#[test]
fn sqfc_no_names_test() {
    // `"#"`
    let sqfc = read(&input("fn_dbSymbolClass.sqfc")).unwrap();
    assert_eq!(sqfc.name_table, None);
    assert_eq!(
        sqfc.code().unwrap().instructions,
        [
            SqfcInstruction::EndStatement,
            SqfcInstruction::Push { constant: 0 }
        ]
    );
    assert_eq!(
        sqfc.source(),
        Some("#line 0 \"\\A3\\Functions_F\\Database\\fn_dbSymbolClass.sqf\"\n\"#\"")
    );
    let mut sorted = sqfc.clone();
    sorted.sort_name_table();
    assert_eq!(sorted.name_table, None);
}

#[test]
fn sqfc_include_test() {
    let sqfc = read(&input("RscDisplayMovieInterrupt.sqfc")).unwrap();
    // Only files with code, not the `.inc` of macros.
    assert_eq!(
        sqfc.files,
        [
            "\\A3\\Ui_f\\scripts\\GUI\\RscDisplayMovieInterrupt.sqf",
            "//temp/bin/A3/Ui_f/scripts/GUI/RscFeedback.sqf"
        ]
    );
    let included = sqfc
        .constants
        .iter()
        .filter_map(|c| match c {
            SqfcConstant::Code(code) => Some(code),
            _ => None,
        })
        .flat_map(|code| &code.instructions)
        .filter_map(SqfcInstruction::location)
        .find(|location| location.file == 1)
        .unwrap();
    assert_eq!(
        sqfc.file(included),
        Some("//temp/bin/A3/Ui_f/scripts/GUI/RscFeedback.sqf")
    );
    assert!(
        sqfc.source()
            .unwrap()
            .contains("#line 1 \"//temp/bin/A3/Ui_f/scripts/GUI/RscFeedback.sqf\"")
    );
    assert_eq!(
        sqfc.file(&SqfcLocation {
            file: 2,
            ..*included
        }),
        None
    );
}

#[test]
fn sqfc_name_table_order_test() {
    // HEMTT keeps the order names appear in, the writer keeps the table's order.
    let mut sqfc = read(&input("fn_spawnGroup.sqfc")).unwrap();
    sqfc.name_table.as_mut().unwrap().reverse();
    let bytes = write(&sqfc);
    assert_eq!(read(&bytes).unwrap(), sqfc);
    assert!(bytes.len() > 4);

    // Missing names are added to the end, sorted.
    let mut partial = sqfc.clone();
    partial.name_table = Some(vec!["select".into(), "unused".into()]);
    let back = read(&write(&partial)).unwrap();
    let table = back.name_table.clone().unwrap();
    assert_eq!(table[..2], ["select", "unused"]);
    assert!(table[2..].is_sorted());
    assert!(!table[2..].contains(&"select".to_string()));
    assert_eq!(back.constants, sqfc.constants);

    // Without a table, the names are in the instructions.
    let mut inline = sqfc.clone();
    inline.name_table = None;
    inline.compressed = false;
    let bytes = write(&inline);
    assert_eq!(read(&bytes).unwrap(), inline);
    assert!(bytes.windows(8).any(|w| w == b"exitwith"));
}

#[test]
fn sqfc_uncompressed_test() {
    let mut sqfc = read(&input("fn_ambientAnim__terminate.sqfc")).unwrap();
    sqfc.compressed = false;
    let bytes = write(&sqfc);
    assert_eq!(read(&bytes).unwrap(), sqfc);
    assert!(bytes.windows(5).any(|w| w == b"_unit"));
}

#[test]
fn sqfc_version_2_test() {
    let location = at(3, 1);
    let mut sqfc = script(
        SQFC_VERSION_2,
        vec![
            SqfcInstruction::EndStatement,
            SqfcInstruction::Push { constant: 2 },
            SqfcInstruction::Push { constant: 3 },
            SqfcInstruction::MakeArray {
                size: 70_000,
                location,
            },
            SqfcInstruction::MakeHashMap { location },
            SqfcInstruction::AssignToLocal {
                name: "_map".into(),
                location,
            },
            SqfcInstruction::CallNular {
                name: "time".into(),
                location,
            },
        ],
        vec![
            SqfcConstant::HashMap {
                flags: 1,
                items: vec![
                    SqfcConstant::String("key".into()),
                    SqfcConstant::Boolean(true),
                ],
            },
            SqfcConstant::Array {
                flags: 1,
                items: vec![SqfcConstant::Scalar(-0.5)],
            },
        ],
    );
    sqfc.sort_name_table();
    assert_eq!(sqfc.name_table, Some(vec!["_map".into(), "time".into()]));
    let bytes = write(&sqfc);
    assert_eq!(read(&bytes).unwrap(), sqfc);
    assert_eq!(sqfc.source(), Some("source"));

    // Not in version 1.
    let mut v1 = Sqfc {
        version: SQFC_VERSION_1,
        ..sqfc.clone()
    };
    v1.constants[0] = SqfcConstant::Code(SqfcCode {
        source: SqfcCodeSource::Constant(1),
        instructions: vec![],
    });
    assert_eq!(write_error(&v1), SqfcError::InvalidHashMap);
    let mut v1 = script(
        SQFC_VERSION_1,
        vec![SqfcInstruction::MakeHashMap { location }],
        vec![],
    );
    assert_eq!(write_error(&v1), SqfcError::InvalidHashMap);
    v1.constants[0] = SqfcConstant::Code(SqfcCode {
        source: SqfcCodeSource::Constant(1),
        instructions: vec![SqfcInstruction::MakeArray {
            size: 70_000,
            location,
        }],
    });
    assert_eq!(write_error(&v1), SqfcError::ArrayTooLarge(70_000));
    v1.constants[0] = SqfcConstant::Array {
        flags: 1,
        items: vec![],
    };
    assert_eq!(write_error(&v1), SqfcError::InvalidFlags(1));
    // Odd hash maps.
    let mut odd = sqfc.clone();
    odd.constants[2] = SqfcConstant::HashMap {
        flags: 0,
        items: vec![SqfcConstant::Boolean(true)],
    };
    assert_eq!(write_error(&odd), SqfcError::InvalidHashMap);
}

#[test]
fn sqfc_accessors_test() {
    let mut sqfc = script(SQFC_VERSION_1, vec![], vec![]);
    let range = SqfcCode {
        source: SqfcCodeSource::Range {
            offset: 2,
            length: 3,
        },
        instructions: vec![],
    };
    assert_eq!(sqfc.code_text(&range), Some("urc"));
    let past = SqfcCode {
        source: SqfcCodeSource::Range {
            offset: 4,
            length: 3,
        },
        instructions: vec![],
    };
    assert_eq!(sqfc.code_text(&past), None);
    let not_a_string = SqfcCode {
        source: SqfcCodeSource::Constant(0),
        instructions: vec![],
    };
    assert_eq!(sqfc.code_text(&not_a_string), None);

    // The code that runs has a range: no source.
    sqfc.constants[0] = SqfcConstant::Code(range);
    assert_eq!(sqfc.source(), None);
    sqfc.constants[0] = SqfcConstant::Code(not_a_string);
    assert_eq!(sqfc.source(), None);
    // Not code.
    sqfc.code_index = 1;
    assert_eq!(sqfc.code(), None);
    assert_eq!(sqfc.source(), None);
    sqfc.code_index = 9;
    assert_eq!(sqfc.code(), None);
}

#[test]
fn sqfc_write_errors_test() {
    let mut sqfc = script(SQFC_VERSION_1, vec![], vec![]);
    sqfc.constants[0] = SqfcConstant::Code(SqfcCode {
        source: SqfcCodeSource::Constant(1 << 63),
        instructions: vec![],
    });
    assert_eq!(write_error(&sqfc), SqfcError::InvalidSource);
    sqfc.constants[0] = SqfcConstant::Code(SqfcCode {
        source: SqfcCodeSource::Range {
            offset: 0,
            length: 0x8000_0000,
        },
        instructions: vec![],
    });
    assert_eq!(write_error(&sqfc), SqfcError::InvalidSource);

    let mut long = script(SQFC_VERSION_1, vec![], vec![]);
    long.constants[1] = SqfcConstant::String("a".repeat(0x100_0000));
    assert_eq!(write_error(&long), SqfcError::StringTooLong(0x100_0000));

    let mut many = script(SQFC_VERSION_1, vec![], vec![]);
    many.constants.resize(0x1_0000, SqfcConstant::Boolean(true));
    assert_eq!(write_error(&many), SqfcError::TooManyConstants);

    let mut files = script(SQFC_VERSION_1, vec![], vec![]);
    files.files = vec![String::new(); 0x1_0000];
    assert_eq!(write_error(&files), SqfcError::TooManyFiles);

    let names = (0..0x1_0000)
        .map(|i| get_variable(&format!("_{i}"), 0, 1))
        .collect();
    let names = script(SQFC_VERSION_1, names, vec![]);
    assert_eq!(write_error(&names), SqfcError::TooManyNames);
}

#[test]
fn sqfc_read_errors_test() {
    assert_eq!(error(read(&2u32.to_le_bytes()[..3])), SqfcError::Truncated);
    assert_eq!(
        error(read(&3u32.to_le_bytes())),
        SqfcError::UnsupportedVersion(3)
    );
    let version = SQFC_VERSION_1.to_le_bytes();
    let with = |blocks: &[u8]| [&version[..], blocks].concat();
    assert_eq!(error(read(&with(&[7]))), SqfcError::UnknownBlock(7));
    assert_eq!(error(read(&with(&[4]))), SqfcError::UnsupportedBlock(4));
    assert_eq!(
        error(read(&with(&[3, 0, 0, 0, 0, 0, 0, 0, 0]))),
        SqfcError::MissingBlock(0)
    );
    assert_eq!(error(read(&with(&[0, 0, 0]))), SqfcError::MissingBlock(3));
    assert_eq!(
        error(read(&with(&[0, 0, 0, 0, 0, 0]))),
        SqfcError::RepeatedBlock(0)
    );
    // The name table comes first.
    let names = compressed(5, &[0, 0]);
    assert_eq!(
        error(read(&with(&[&[0, 0, 0][..], &names].concat()))),
        SqfcError::RepeatedBlock(5)
    );
    // No locations block is no files.
    let empty = read(&raw(&[0, 0])).unwrap();
    assert!(empty.files.is_empty());
    assert_eq!(empty.name_table, None);
    assert!(!empty.compressed);

    // Compressed blocks.
    assert_eq!(
        error(read(&with(&[1, 2, 0, 0, 0, 3]))),
        SqfcError::UnknownCompression(3)
    );
    assert!(matches!(
        error(read(&with(&[1, 2, 0, 0, 0, 2, 0xff]))),
        SqfcError::Lzo(_)
    ));
    let mut short = compressed(1, &[0, 0]);
    short[1] = 3;
    assert!(matches!(
        error(read(&with(&short))),
        SqfcError::Lzo(e) if e.contains("expected 3")
    ));
    assert_eq!(
        error(read(&with(&compressed(1, &[0, 0, 9])))),
        SqfcError::TrailingData(1)
    );
    assert_eq!(
        error(read(&with(&compressed(5, &[1, 0])))),
        SqfcError::Truncated
    );

    // Constants.
    assert_eq!(error(read(&raw(&[1, 0, 9]))), SqfcError::UnknownConstant(9));
    assert_eq!(
        error(read(&raw(&[1, 0, 3, 2]))),
        SqfcError::InvalidBoolean(2)
    );
    assert_eq!(
        error(read(&raw(&[&[1, 0, 1][..], &[1, 0, 0, 0xff]].concat()))),
        SqfcError::InvalidString
    );
    assert_eq!(
        error(read(&raw(&[1, 0, 6, 0, 0, 0, 0]))),
        SqfcError::InvalidHashMap
    );
    let mut v2 = raw(&[1, 0, 6, 1, 0, 0, 0, 0]);
    v2[0] = 2;
    assert_eq!(error(read(&v2)), SqfcError::InvalidHashMap);

    // Instructions.
    let code = |instructions: &[u8]| {
        let mut out = vec![1, 0, 0];
        out.extend(0u64.to_le_bytes());
        out.extend(1u32.to_le_bytes());
        out.extend(instructions);
        out
    };
    assert_eq!(
        error(read(&raw(&code(&[10])))),
        SqfcError::UnknownInstruction(10)
    );
    assert_eq!(
        error(read(&raw(&code(&[9])))),
        SqfcError::UnknownInstruction(9)
    );
    let location = [0, 0, 0, 0, 0, 1, 0];
    let inline = raw(&code(&[&[7][..], &location, &string("_x")].concat()));
    assert_eq!(
        read(&inline).unwrap().code().unwrap().instructions,
        [get_variable("_x", 0, 1)]
    );
    let mut table = with(&compressed(5, &[&[1, 0][..], &string("_x")].concat()));
    table.extend(&raw(&code(&[&[7][..], &location, &[1, 0]].concat()))[4..]);
    assert_eq!(error(read(&table)), SqfcError::InvalidName(1));
    assert_eq!(
        error(read(&with(&compressed(1, &code(&[2, 0]))))),
        SqfcError::Truncated
    );
}
