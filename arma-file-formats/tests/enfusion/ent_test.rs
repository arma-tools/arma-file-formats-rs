//! Black box tests for the binary world (`.ent`) reader and writer.
//!
//! `ent_probes/probe.ent` was written by [`probe_world`] and loaded by Workbench, see
//! `ent_probes/README.md`. The other inputs are assembled by hand from the layout of the game's
//! worlds.

use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use arma_file_formats::{
    enfusion::{
        ent::{Body, Ent, Entry, Object, ObjectId, Value},
        pak::PakReader,
    },
    errors::{AffError, EntError},
};

const PROBE_DIR: &str = "./tests/enfusion/ent_probes";

fn read(bytes: &[u8]) -> Result<Ent, AffError> {
    Ent::from(&mut Cursor::new(bytes))
}

fn write(ent: &Ent) -> Vec<u8> {
    let mut out = Vec::new();
    ent.write(&mut out).unwrap();
    out
}

fn ent_error(result: Result<impl std::fmt::Debug, AffError>) -> EntError {
    match result {
        Err(AffError::EntError(e)) => e,
        other => panic!("expected an EntError, got {other:?}"),
    }
}

/// `EBIN`, version 1, the tables and the tokens.
fn ebin(strings: &[&str], guids: &[u64], tokens: &[u8]) -> Vec<u8> {
    let mut out = b"EBIN".to_vec();
    out.extend(1u32.to_le_bytes());
    out.extend((strings.len() as u16).to_le_bytes());
    out.extend((guids.len() as u16).to_le_bytes());
    for string in strings {
        out.extend((string.len() as u16).to_le_bytes());
        out.extend(string.as_bytes());
    }
    for guid in guids {
        out.extend(guid.to_le_bytes());
    }
    out.extend(tokens);
    out
}

/// Token builders.
fn word(index: u16) -> Vec<u8> {
    [&[0x0d][..], &index.to_le_bytes()].concat()
}

fn block(contents: &[u8]) -> Vec<u8> {
    [
        &[0x0e][..],
        &(contents.len() as u32 + 4).to_le_bytes(),
        contents,
    ]
    .concat()
}

const INSTANCE: [u8; 3] = [0x00, 0, 0];

fn name(name: &str) -> Vec<u8> {
    [
        &[0x43][..],
        &(name.len() as u16).to_le_bytes(),
        name.as_bytes(),
    ]
    .concat()
}

fn vec3(v: [f32; 3]) -> Vec<u8> {
    let mut out = vec![0x08];
    v.iter().for_each(|f| out.extend(f.to_le_bytes()));
    out
}

const PREFAB: u64 = 0xE0A0_5C76_552E_7F58;
const SOUND: u64 = 0x5EEB_C743_F318_F376;

/// The example of the module docs:
///
/// ```text
/// SCR_AIWorld : "{E0A05C76552E7F58}" { coords 107.9 1 159.5 }
/// $grp GenericEntity {
///  First { coords 1 2 3 { GenericEntity Child { } } }
///  { Filenames { "{5EEBC743F318F376}" } }
/// }
/// ```
fn docs_example() -> Vec<u8> {
    let tokens = [
        word(0),
        word(1),
        vec![0x05, 0, 0],
        block(&[&INSTANCE[..], &word(2), &vec3([107.9, 1.0, 159.5])].concat()),
        word(3),
        word(4),
        block(
            &[
                name("First"),
                block(
                    &[
                        &INSTANCE[..],
                        &word(2),
                        &vec3([1.0, 2.0, 3.0]),
                        &block(&[word(4), name("Child"), block(&INSTANCE)].concat()),
                    ]
                    .concat(),
                ),
                block(&[&INSTANCE[..], &word(5), &block(&[0x85, 1, 0, 0, 0, 1, 0])].concat()),
            ]
            .concat(),
        ),
    ]
    .concat();
    ebin(
        &[
            "SCR_AIWorld",
            ":",
            "coords",
            "$grp",
            "GenericEntity",
            "Filenames",
        ],
        &[PREFAB, SOUND],
        &tokens,
    )
}

fn object(key: Option<&str>, class: Option<&str>, entries: Vec<Entry>) -> Object {
    Object {
        key: key.map(str::to_owned),
        class: class.map(str::to_owned),
        body: Body::Entries {
            instance: Some(0),
            entries,
        },
        ..Object::default()
    }
}

fn block_object(key: Option<&str>, entries: Vec<Entry>) -> Object {
    Object {
        key: key.map(str::to_owned),
        body: Body::Entries {
            instance: None,
            entries,
        },
        ..Object::default()
    }
}

fn property(key: &str, value: Value) -> Entry {
    Entry::Property {
        key: key.to_owned(),
        value,
    }
}

fn named(mut object: Object, name: &str) -> Object {
    object.name = Some(name.to_owned());
    object
}

#[test]
fn reads_docs_example() {
    let bytes = docs_example();
    let ent = read(&bytes).unwrap();

    let ai_world = Object {
        parent: Some(PREFAB),
        ..object(
            Some("SCR_AIWorld"),
            None,
            vec![property("coords", Value::Vec3([107.9, 1.0, 159.5]))],
        )
    };
    let child = named(object(Some("GenericEntity"), None, vec![]), "Child");
    let first = named(
        object(
            None,
            None,
            vec![
                property("coords", Value::Vec3([1.0, 2.0, 3.0])),
                Entry::Object(block_object(None, vec![Entry::Object(child)])),
            ],
        ),
        "First",
    );
    let anonymous = object(
        None,
        None,
        vec![Entry::Object(block_object(
            Some("Filenames"),
            vec![Entry::Value(Value::ResourceArray(vec![SOUND]))],
        ))],
    );
    let group = Object {
        class: Some("GenericEntity".to_owned()),
        ..block_object(
            Some("$grp"),
            vec![Entry::Object(first), Entry::Object(anonymous)],
        )
    };
    assert_eq!(
        ent,
        Ent {
            version: 1,
            entries: vec![Entry::Object(ai_world), Entry::Object(group)],
        }
    );
    assert_eq!(write(&ent), bytes);
}

#[test]
fn entities_resolve_groups_and_children() {
    let ent = read(&docs_example()).unwrap();
    let entities = ent.entities();
    let summary: Vec<_> = entities
        .iter()
        .map(|e| (e.class, e.name, e.prefab, e.parent))
        .collect();
    assert_eq!(
        summary,
        [
            ("SCR_AIWorld", None, Some(PREFAB), None),
            ("GenericEntity", Some("First"), None, None),
            ("GenericEntity", Some("Child"), None, Some(1)),
            ("GenericEntity", None, None, None),
        ]
    );
    assert_eq!(entities[0].object.coords(), Some([107.9, 1.0, 159.5]));
    assert_eq!(entities[1].object.coords(), Some([1.0, 2.0, 3.0]));
    assert_eq!(entities[2].object.coords(), None);
    assert_eq!(
        entities[3].object.get("Filenames"),
        Some(&Value::ResourceArray(vec![SOUND]))
    );
    assert_eq!(ent.sub_scene_parent(), None);
}

#[test]
fn group_prefab_and_sub_scene() {
    let other = 0x1B76_F75A_3175_E85C;
    let world = 0xA701_424D_7002_2078;
    let ent = Ent {
        version: 1,
        entries: vec![
            Entry::Object(object(
                Some("SubScene"),
                None,
                vec![property("Parent", Value::Resource(world))],
            )),
            Entry::Object(Object {
                class: Some("GenericEntity".to_owned()),
                parent: Some(PREFAB),
                ..block_object(
                    Some("$grp"),
                    vec![
                        Entry::Object(object(None, None, vec![])),
                        Entry::Object(Object {
                            parent: Some(other),
                            ..named(object(None, None, vec![]), "Own")
                        }),
                    ],
                )
            }),
        ],
    };
    assert_eq!(ent.sub_scene_parent(), Some(world));
    let prefabs: Vec<_> = ent.entities().iter().map(|e| (e.name, e.prefab)).collect();
    assert_eq!(prefabs, [(None, Some(PREFAB)), (Some("Own"), Some(other))]);
    assert_eq!(read(&write(&ent)).unwrap(), ent);
}

/// Every value type, ids and a raw body survive a roundtrip, the tables are in the order of
/// first use and hold each string and GUID once.
#[test]
fn writes_all_value_types() {
    let values = vec![
        property("Int", Value::Int(-1)),
        property("Flags", Value::Flags(0x8_0000_0000_0003)),
        property("Resource", Value::Resource(0)),
        property("Float", Value::Float(0.9)),
        property("Vec2", Value::Vec2([1.0, 2.0])),
        property("Vec3", Value::Vec3([1.0, 2.0, 3.0])),
        property("Vec4", Value::Vec4([1.0, 2.0, 3.0, 4.0])),
        property("Bool", Value::Bool(true)),
        property("String", Value::String("Autotest World\n\nÜ".to_owned())),
        Entry::Value(Value::IntArray(vec![])),
        Entry::Value(Value::FlagsArray(vec![1, u64::MAX])),
        Entry::Value(Value::ResourceArray(vec![SOUND, 0, SOUND])),
        Entry::Value(Value::FloatArray(vec![1.0, 1.5])),
        Entry::Value(Value::Vec2Array(vec![[1.0, 2.0]])),
        Entry::Value(Value::Vec3Array(vec![[1.0, 2.0, 3.0]; 2])),
        Entry::Value(Value::Vec4Array(vec![[0.5; 4]])),
        Entry::Value(Value::BoolArray(vec![false, true])),
        Entry::Value(Value::StringArray(vec![
            "Disassembly".to_owned(),
            String::new(),
        ])),
    ];
    let component = Object {
        id: Some(ObjectId::Shared(0x56B2_B479_3051_E7C9)),
        ..object(Some("RigidBody"), None, values)
    };
    let point = Object {
        id: Some(ObjectId::Inline(0x54C4_E856_2DD8_D20D)),
        ..object(Some("ShapePoint"), None, vec![])
    };
    let spawn_logic = Object {
        id: Some(ObjectId::Shared(0x56B2_B479_3051_E7C9)),
        ..object(Some("m_SpawnLogic"), Some("SCR_SpawnLogic"), vec![])
    };
    let bsp = Object {
        key: Some("BSP".to_owned()),
        body: Body::Raw(b"428 1 41 1 256 ".to_vec()),
        ..Object::default()
    };
    let ent = Ent {
        version: 1,
        entries: vec![Entry::Object(Object {
            parent: Some(SOUND),
            ..named(
                object(
                    Some("GenericEntity"),
                    None,
                    vec![
                        Entry::Object(bsp),
                        Entry::Object(block_object(
                            Some("components"),
                            vec![Entry::Object(component), Entry::Object(spawn_logic)],
                        )),
                        Entry::Object(block_object(Some("Points"), vec![Entry::Object(point)])),
                    ],
                ),
                "Everything",
            )
        })],
    };
    let bytes = write(&ent);
    assert_eq!(read(&bytes).unwrap(), ent);

    let strings = [
        "GenericEntity",
        ":",
        "BSP",
        "components",
        "RigidBody",
        "Int",
        "Flags",
        "Resource",
        "Float",
        "Vec2",
        "Vec3",
        "Vec4",
        "Bool",
        "String",
        "m_SpawnLogic",
        "SCR_SpawnLogic",
        "Points",
        "ShapePoint",
    ];
    let guids = [SOUND, 0x56B2_B479_3051_E7C9, 0];
    let tables = ebin(&strings, &guids, &[]);
    assert_eq!(bytes[..tables.len()], tables);
}

#[test]
fn raw_bodies() {
    let tokens = [
        word(0),
        name("world"),
        block(&[&INSTANCE[..], &word(1), &block(b"255 0 0 1")].concat()),
    ]
    .concat();
    let ent = read(&ebin(&["GenericWorldEntity", "BSP"], &[], &tokens)).unwrap();
    let bsp = ent.entities()[0].object.object("BSP").unwrap();
    assert_eq!(bsp.body, Body::Raw(b"255 0 0 1".to_vec()));
    assert!(bsp.entries().is_empty());

    // An empty block is an empty token stream.
    let ent = read(&ebin(&["Empty"], &[], &[word(0), block(&[])].concat())).unwrap();
    assert_eq!(
        ent.entries,
        [Entry::Object(block_object(Some("Empty"), vec![]))]
    );

    // Raw data starting with a token tag would read back as tokens.
    let mut raw = ent.clone();
    let Entry::Object(object) = &mut raw.entries[0] else {
        unreachable!()
    };
    object.body = Body::Raw(vec![0x0d, 0, 0]);
    assert!(matches!(
        ent_error(raw.write(&mut Vec::new())),
        EntError::Unwritable(_)
    ));
}

#[test]
fn accessors() {
    let mut ent = read(&docs_example()).unwrap();
    let Entry::Object(group) = &mut ent.entries[1] else {
        unreachable!()
    };
    assert_eq!(group.object("Filenames"), None);
    let anonymous = match &group.entries()[1] {
        Entry::Object(object) => object,
        other => panic!("{other:?}"),
    };
    assert_eq!(anonymous.object("Filenames").unwrap().entries().len(), 1);
    assert_eq!(anonymous.get("Missing"), None);

    group.entries_mut().unwrap().clear();
    assert_eq!(ent.entities().len(), 1);
    assert_eq!(ObjectId::Shared(5).id(), ObjectId::Inline(5).id());
}

#[test]
fn errors() {
    assert_eq!(
        ent_error(read(b"FORM\0\0\0\0")),
        EntError::UnknownMagic(b"FORM".to_vec())
    );
    assert_eq!(
        ent_error(read(b"EB")),
        EntError::UnknownMagic(b"EB".to_vec())
    );

    let valid = docs_example();
    for len in [8, 13, valid.len() - 1] {
        assert!(read(&valid[..len]).is_err(), "truncated to {len}");
    }

    let unknown = ebin(&["Key"], &[], &[word(0), vec![0x03, 0]].concat());
    assert_eq!(
        ent_error(read(&unknown)),
        EntError::UnknownToken {
            tag: 0x03,
            offset: 20
        }
    );
    let unknown_array = ebin(&[], &[], &[0x8d, 0, 0, 0, 0]);
    assert!(matches!(
        ent_error(read(&unknown_array)),
        EntError::UnknownToken { tag: 0x8d, .. }
    ));

    let malformed = [
        ebin(&[], &[], &word(0)),
        ebin(&["Key"], &[], &[0x05, 0, 0]),
        ebin(&["Key"], &[], &word(0)),
        ebin(
            &["Key"],
            &[],
            &[word(0), block(&[&INSTANCE[..], &INSTANCE].concat())].concat(),
        ),
        ebin(&["Key"], &[], &[word(0), vec![0x0e, 3, 0, 0, 0]].concat()),
        ebin(&["Key"], &[], &[word(0), vec![0x0e, 9, 0, 0, 0]].concat()),
        ebin(&["Key"], &[], &[word(0), name("Name"), word(0)].concat()),
        ebin(&["Key", ":"], &[], &[word(0), word(1), vec![0x01]].concat()),
        // A key without value at the end of its block.
        ebin(
            &["Key"],
            &[],
            &[word(0), block(&word(0)), vec![0, 0, 0, 0]].concat(),
        ),
    ];
    for (i, bytes) in malformed.iter().enumerate() {
        assert!(read(bytes).is_err(), "malformed {i}");
    }

    let huge_array = ebin(&[], &[], &[0x81, 0xff, 0xff, 0xff, 0xff]);
    assert!(matches!(
        ent_error(read(&huge_array)),
        EntError::Truncated(_)
    ));
    let bad_utf8 = ebin(&[], &[], &[0x0c, 1, 0, 0xff]);
    assert!(matches!(
        ent_error(read(&bad_utf8)),
        EntError::Malformed { .. }
    ));

    let long = Ent {
        version: 1,
        entries: vec![Entry::Value(Value::String("x".repeat(0x1_0000)))],
    };
    assert_eq!(
        ent_error(long.write(&mut Vec::new())),
        EntError::TooLarge("string")
    );
    let many_keys = Ent {
        version: 1,
        entries: (0..=0x1_0000)
            .map(|i| property(&i.to_string(), Value::Int(i)))
            .collect(),
    };
    assert_eq!(
        ent_error(many_keys.write(&mut Vec::new())),
        EntError::TooLarge("string table")
    );
}

/// The probe of `ent_probes/README.md`.
fn probe_world() -> Ent {
    let entity = |name: &str, coords: [f32; 3], extra: Vec<Entry>| {
        let mut entries = vec![property("coords", Value::Vec3(coords))];
        entries.extend(extra);
        named(object(Some("GenericEntity"), None, entries), name)
    };
    let group = |entries: Vec<Object>| Object {
        class: Some("GenericEntity".to_owned()),
        ..block_object(
            Some("$grp"),
            entries.into_iter().map(Entry::Object).collect(),
        )
    };
    let group_entry = |name: Option<&str>, coords: [f32; 3]| {
        let object = object(None, None, vec![property("coords", Value::Vec3(coords))]);
        match name {
            Some(name) => named(object, name),
            None => object,
        }
    };
    let children = block_object(
        None,
        vec![
            Entry::Object(entity("EntProbe_Child", [1.0, 2.0, 3.0], vec![])),
            Entry::Object(group(vec![group_entry(
                Some("EntProbe_GroupChild"),
                [4.0, 5.0, 6.0],
            )])),
        ],
    );
    Ent {
        version: 1,
        entries: vec![
            Entry::Object(entity(
                "EntProbe_Single",
                [10.0, 20.0, 30.0],
                vec![
                    property("angles", Value::Vec3([0.0, 90.0, 0.0])),
                    property("scale", Value::Float(2.5)),
                ],
            )),
            Entry::Object(group(vec![
                group_entry(Some("EntProbe_GroupA"), [11.0, 21.0, 31.0]),
                group_entry(None, [12.0, 22.0, 32.0]),
            ])),
            Entry::Object(entity(
                "EntProbe_Parent",
                [100.0, 0.0, 100.0],
                vec![Entry::Object(children)],
            )),
        ],
    }
}

/// Workbench loaded `probe.ent` with these entities, see `ent_probes/README.md`. Set
/// `AFF_ENT_PROBE_OUT` to a path to write the probe there.
#[test]
fn probe_test() {
    let probe = probe_world();
    if let Ok(path) = std::env::var("AFF_ENT_PROBE_OUT") {
        std::fs::write(path, write(&probe)).unwrap();
    }
    let bytes = std::fs::read(format!("{PROBE_DIR}/probe.ent")).unwrap();
    assert_eq!(write(&probe), bytes);
    let ent = read(&bytes).unwrap();
    assert_eq!(ent, probe);

    let entities = ent.entities();
    let summary: Vec<_> = entities
        .iter()
        .map(|e| {
            let parent = e.parent.map(|p| entities[p].name.unwrap_or_default());
            (
                e.class,
                e.name.unwrap_or_default(),
                e.object.coords().unwrap(),
                parent,
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("GenericEntity", "EntProbe_Single", [10.0, 20.0, 30.0], None),
            ("GenericEntity", "EntProbe_GroupA", [11.0, 21.0, 31.0], None),
            ("GenericEntity", "", [12.0, 22.0, 32.0], None),
            (
                "GenericEntity",
                "EntProbe_Parent",
                [100.0, 0.0, 100.0],
                None
            ),
            (
                "GenericEntity",
                "EntProbe_Child",
                [1.0, 2.0, 3.0],
                Some("EntProbe_Parent")
            ),
            (
                "GenericEntity",
                "EntProbe_GroupChild",
                [4.0, 5.0, 6.0],
                Some("EntProbe_Parent")
            ),
        ]
    );
    assert_eq!(entities[0].object.get("scale"), Some(&Value::Float(2.5)));
}

/// Reads every `.ent` in the paks below `AFF_ENT_SCAN_DIR`, e.g. the game's `addons`
/// directory: checks that it writes back identically and that its entities are well formed.
#[test]
fn ent_scan_dir_test() {
    fn visit(path: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, files);
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pak"))
            {
                files.push(path);
            }
        }
    }

    let Ok(dir) = std::env::var("AFF_ENT_SCAN_DIR") else {
        return;
    };

    let mut paks = Vec::new();
    visit(Path::new(&dir), &mut paks);
    assert!(!paks.is_empty(), "no pak files in {dir}");

    let mut count = 0;
    for pak in paks {
        let reader = PakReader::from_stream(BufReader::new(File::open(&pak).unwrap())).unwrap();
        for entry in &reader.pak.entries {
            if !entry.path.ends_with(".ent") {
                continue;
            }
            let bytes = reader.read_entry(entry).unwrap();
            let ent = read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", entry.path));
            assert_eq!(write(&ent), bytes, "{}", entry.path);

            let entities = ent.entities();
            for (i, entity) in entities.iter().enumerate() {
                assert!(entity.parent.is_none_or(|p| p < i), "{}", entry.path);
                assert!(
                    matches!(
                        entity.object.body,
                        Body::Entries {
                            instance: Some(0),
                            ..
                        }
                    ),
                    "{}: {}",
                    entry.path,
                    entity.class
                );
            }
            println!(
                "{}: {} entities, sub scene of {:?}",
                entry.path,
                entities.len(),
                ent.sub_scene_parent().map(|guid| format!("{guid:016X}"))
            );
            count += 1;
        }
    }
    assert!(count > 0, "no ent files in {dir}");
}
