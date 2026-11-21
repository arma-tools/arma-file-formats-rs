//! Tests for the generic Enfusion text parser on small synthetic documents.

use arma_file_formats::{
    enfusion::text::{Entry, Object, Value},
    errors::{AffError, TextError},
};

fn word(text: &str) -> Value {
    Value::Word(text.to_string())
}

fn string(text: &str) -> Value {
    Value::String(text.to_string())
}

/// Line and message of a parse error.
fn parse_error(text: &str) -> (usize, String) {
    match Object::parse(text) {
        Err(AffError::TextError(TextError::Parse { line, message })) => (line, message),
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn text_root_and_accessors() {
    let text = "\u{feff}\n\nMatPBRBasic \"{GUID}\" : \"{PARENT}base.emat\" {\n Scale 1.5\n Name \"text\"\n Anim anim(20 \"x\")\n Flag yes\n Refs {\n  \"a\"\n }\n}\n\n";
    let object = Object::parse(text).unwrap();
    assert_eq!(object.class.as_deref(), Some("MatPBRBasic"));
    assert_eq!(object.id.as_deref(), Some("{GUID}"));
    assert_eq!(object.parent.as_deref(), Some("{PARENT}base.emat"));

    assert_eq!(object.float("Scale"), Some(1.5));
    assert_eq!(object.float("Flag"), None);
    assert_eq!(object.float("Name"), None);
    assert_eq!(object.float("Missing"), None);
    assert_eq!(object.values("Name"), Some(&[string("text")][..]));
    assert_eq!(object.values("Name").unwrap()[0].as_str(), Some("text"));
    assert_eq!(object.values("Flag").unwrap()[0].as_str(), Some("yes"));

    let call = &object.values("Anim").unwrap()[0];
    assert_eq!(
        call,
        &Value::Call {
            name: "anim".to_string(),
            args: vec![word("20"), string("x")],
        }
    );
    assert_eq!(call.as_str(), None);
    assert_eq!(call.as_f32(), None);

    let refs = object.object("Refs").unwrap();
    assert_eq!(refs.class, None);
    assert_eq!(refs.get("a").unwrap().values, []);
    assert!(object.object("Scale").is_none());
}

#[test]
fn text_strings() {
    let object = Object::parse(
        "C {\n A \"quote \\\" and \\\\ kept\"\n B \"first \"\\\n    \"second\"\n C \"multi\nline\"\n D \"x\"\\ y\n E \"x\"\\\n F\n}",
    )
    .unwrap();
    assert_eq!(
        object.values("A"),
        Some(&[string("quote \" and \\\\ kept")][..])
    );
    // A `\` at the end of a line joins the next string.
    assert_eq!(object.values("B"), Some(&[string("first second")][..]));
    assert_eq!(object.values("C"), Some(&[string("multi\nline")][..]));
    // Not followed by a newline, or the next line isn't a string: the `\` is a word.
    assert_eq!(
        object.values("D"),
        Some(&[string("x"), word("\\"), word("y")][..])
    );
    assert_eq!(object.values("E"), Some(&[string("x"), word("\\")][..]));
    assert!(object.get("F").is_some());
}

#[test]
fn text_line_numbers_count_lines_in_strings() {
    // The string spans lines 2 and 3, the error is on line 4.
    assert_eq!(
        parse_error("C {\n A \"one\ntwo\"\n B )\n}"),
        (4, "expected a value, found Some(CloseParen)".to_string())
    );
    assert_eq!(parse_error("C {\n B \"a\"\\\n \"b\"\n )\n}").0, 4);
}

#[test]
fn text_missing_closing_braces_end_at_the_file_end() {
    let object = Object::parse("C {\n Inner {\n  A 1\n").unwrap();
    assert_eq!(object.object("Inner").unwrap().float("A"), Some(1.0));
}

#[test]
fn text_nested_headers_keyless_blocks_and_parents() {
    let object = Object::parse(
        "C {\n Key MatUVTransform \"{ID}\" extra \"{ID2}\" {\n  A 1\n }\n Ent GenericEntity : \"{P}prefab.et\" {\n }\n {\n  Child {\n  }\n }\n}",
    )
    .unwrap();
    let key = object.object("Key").unwrap();
    assert_eq!(key.class.as_deref(), Some("MatUVTransform"));
    assert_eq!(key.id.as_deref(), Some("{ID}"));
    assert_eq!(key.float("A"), Some(1.0));
    assert!(object.get("Key").unwrap().values.is_empty());

    let ent = object.object("Ent").unwrap();
    assert_eq!(ent.class.as_deref(), Some("GenericEntity"));
    assert_eq!(ent.parent.as_deref(), Some("{P}prefab.et"));

    let keyless = object.get("").unwrap();
    assert!(keyless.values.is_empty());
    assert!(keyless.object.as_ref().unwrap().object("Child").is_some());
}

#[test]
fn text_parse_errors() {
    for (text, line, message) in [
        ("{", 1, "expected a class name, found Some(OpenBrace)"),
        ("", 1, "expected a class name, found None"),
        ("C : 5 {", 1, "expected a parent, found Some(Word(\"5\"))"),
        (
            "C \"a\" \"b\" {",
            1,
            "expected `{`, found Some(String(\"b\"))",
        ),
        ("C\n{", 1, "expected `{`, found Some(Newline)"),
        (
            "C { }\nD { }",
            2,
            "unexpected Word(\"D\") after the root object",
        ),
        ("C {\n ( }", 2, "expected a key, found Some(OpenParen)"),
        (
            "C {\n K : 5\n}",
            2,
            "expected a parent, found Some(Word(\"5\"))",
        ),
        (
            "C {\n K anim(1 {\n}",
            2,
            "expected `)`, found Some(OpenBrace)",
        ),
        (
            "C {\n K )\n}",
            2,
            "expected a value, found Some(CloseParen)",
        ),
    ] {
        assert_eq!(parse_error(text), (line, message.to_string()), "{text:?}");
    }
}

#[test]
fn text_merge() {
    let mut base = Object::parse(
        "Mat {\n Color 1 1 1\n Plain {\n  A 1\n  B 2\n }\n UV MatUVTransform {\n  Scale 1\n  Offset 0\n }\n}",
    )
    .unwrap();
    let child = Object::parse(
        "MatChild : \"{P}base.emat\" {\n Color 0 0 0\n Plain {\n  A 3\n }\n UV MatUVTransform \"{ID}\" {\n  Scale 2\n }\n New Thing {\n  X 1\n }\n Extra 5\n}",
    )
    .unwrap();
    base.merge(child);

    assert_eq!(base.class.as_deref(), Some("MatChild"));
    assert_eq!(
        base.values("Color"),
        Some(&[word("0"), word("0"), word("0")][..])
    );
    // Plain blocks are replaced as a whole.
    let plain = base.object("Plain").unwrap();
    assert_eq!(plain.float("A"), Some(3.0));
    assert_eq!(plain.get("B"), None);
    // Objects with a class are merged.
    let uv = base.object("UV").unwrap();
    assert_eq!(uv.id.as_deref(), Some("{ID}"));
    assert_eq!(uv.float("Scale"), Some(2.0));
    assert_eq!(uv.float("Offset"), Some(0.0));
    // New entries are added, with or without an object.
    assert_eq!(base.object("New").unwrap().class.as_deref(), Some("Thing"));
    assert_eq!(base.float("Extra"), Some(5.0));

    // A child without a class keeps the base's.
    let mut base = Object::parse("Mat {\n}").unwrap();
    base.merge(Object {
        entries: vec![Entry {
            key: "K".to_string(),
            values: vec![word("1")],
            object: None,
        }],
        ..Object::default()
    });
    assert_eq!(base.class.as_deref(), Some("Mat"));
    assert_eq!(base.float("K"), Some(1.0));
}
