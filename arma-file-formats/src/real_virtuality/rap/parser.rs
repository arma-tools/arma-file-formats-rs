//! Parser for preprocessed text configs.
//!
//! Values are stored the way the game's binarizer stores them:
//! - integers are `Long` (`Int64` when they don't fit), hexadecimal (`0x10`) included
//! - numbers with a fraction or an exponent are `Float`, also `1.0`
//! - everything else without quotes is a `String` holding the text as written, for example
//!   `true`, `$STR_key` or `0.2 * 4 / 3`

//!
//! It also accepts what the game loads although it's not quite right: a missing `;` at the
//! end of a line, before `}` or after a class, `;` instead of `,` in arrays, empty values and
//! a stray `}` at the end.

use crate::{
    errors::AffError,
    real_virtuality::rap::{CfgClass, CfgEntry, CfgEnumValue, CfgProperty, CfgValue},
};
use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use std::io::Cursor;

struct ParseError {
    /// Byte offset
    pos: usize,
    msg: String,
}

struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    i: usize,
    enums: Vec<CfgEnumValue>,
}

/// The entries and enum values of a config.
pub fn parse(src: &str) -> Result<(Vec<CfgEntry>, Vec<CfgEnumValue>), AffError> {
    let mut parser = Parser {
        src,
        bytes: src.as_bytes(),
        i: 0,
        enums: Vec::new(),
    };
    let entries = parser
        .entries(true)
        .map_err(|err| AffError::ParseError(report(src, &err)))?;
    Ok((entries, parser.enums))
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.i).copied()
    }

    fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError {
            pos: self.i,
            msg: msg.into(),
        }
    }

    /// What is at the current position, for error messages
    fn found(&self) -> String {
        self.src[self.i..]
            .chars()
            .next()
            .map_or_else(|| "end of file".to_owned(), |c| format!("`{c}`"))
    }

    fn expect(&mut self, b: u8) -> Result<(), ParseError> {
        self.ws();
        if self.eat(b) {
            Ok(())
        } else {
            Err(self.err(format!("expected `{}`, found {}", b as char, self.found())))
        }
    }

    /// Skips whitespace and `\` at the end of lines (comments are gone after preprocessing).
    /// Returns whether it skipped a line break.
    fn ws(&mut self) -> bool {
        let mut newline = false;
        loop {
            while let Some(b) = self.peek().filter(u8::is_ascii_whitespace) {
                newline |= b == b'\n';
                self.i += 1;
            }
            let rest = &self.bytes[self.i..];
            if rest.starts_with(b"\\\n") || rest.starts_with(b"\\\r\n") {
                self.i += 1;
            } else {
                return newline;
            }
        }
    }

    /// Class and property names: letters, digits and `_`, also at the start (`30Rnd_mag`).
    fn name(&mut self) -> Option<&str> {
        let start = self.i;
        let len = self.src[start..]
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(self.src.len() - start);
        self.i += len;
        (len > 0).then(|| &self.src[start..start + len])
    }

    fn expect_name(&mut self, what: &str) -> Result<String, ParseError> {
        self.ws();
        let name = self.name().map(str::to_owned);
        name.ok_or_else(|| self.err(format!("expected {what}, found {}", self.found())))
    }

    /// A missing `;` at the end of a line, before `}` or at the end of the file is accepted,
    /// like the game does.
    fn end_statement(&mut self) -> Result<(), ParseError> {
        let newline = self.ws();
        if self.eat(b';') || newline || matches!(self.peek(), Some(b'}') | None) {
            Ok(())
        } else {
            Err(self.err(format!("expected `;`, found {}", self.found())))
        }
    }

    fn entries(&mut self, top: bool) -> Result<Vec<CfgEntry>, ParseError> {
        let mut entries = Vec::new();
        loop {
            self.ws();
            match self.peek() {
                None if top => return Ok(entries),
                None => return Err(self.err("expected `}`, found end of file")),
                Some(b'}') if !top => return Ok(entries),
                // A stray `}` at the top is ignored, like a stray `;`
                Some(b';' | b'}') => self.i += 1,
                Some(_) => {
                    if let Some(entry) = self.entry()? {
                        entries.push(entry);
                    }
                }
            }
        }
    }

    fn entry(&mut self) -> Result<Option<CfgEntry>, ParseError> {
        let start = self.i;
        let Some(word) = self.name() else {
            return Err(self.err(format!(
                "expected a class or property, found {}",
                self.found()
            )));
        };
        let word = word.to_owned();
        self.ws();
        let keyword = !matches!(self.peek(), Some(b'=' | b'[' | b'+'));

        if keyword && word.eq_ignore_ascii_case("class") {
            let name = self.expect_name("a class name")?;
            self.ws();
            if self.eat(b';') {
                return Ok(Some(CfgEntry::Extern(name)));
            }
            let parent = if self.eat(b':') {
                Some(self.expect_name("a parent class name")?)
            } else {
                None
            };
            self.expect(b'{')?;
            let entries = self.entries(false)?;
            self.expect(b'}')?;
            // The game also loads classes without `;` after `}`
            self.ws();
            self.eat(b';');
            return Ok(Some(CfgEntry::Class(CfgClass {
                name,
                parent,
                entries,
            })));
        }

        if keyword && word.eq_ignore_ascii_case("delete") {
            let name = self.expect_name("a class name")?;
            self.end_statement()?;
            return Ok(Some(CfgEntry::Delete(name)));
        }

        if word.eq_ignore_ascii_case("enum") && self.peek() == Some(b'{') {
            self.enum_values()?;
            self.ws();
            self.eat(b';');
            return Ok(None);
        }

        // `__EXEC(...)` runs code while the game binarizes, there is nothing to store
        if word == "__EXEC" && self.peek() == Some(b'(') {
            self.raw_until(b"");
            self.end_statement()?;
            return Ok(None);
        }

        let array = if self.eat(b'[') {
            self.expect(b']')?;
            self.ws();
            true
        } else {
            false
        };
        let append = array && self.bytes[self.i..].starts_with(b"+=");
        if append {
            self.i += 2;
        } else if !self.eat(b'=') {
            self.i = start + word.len();
            self.ws();
            return Err(self.err(format!(
                "expected `=`{}, found {}",
                if array { " or `+=`" } else { "" },
                self.found()
            )));
        }
        self.ws();
        let value = if array || self.peek() == Some(b'{') {
            self.array()?
        } else {
            self.value()?
        };
        self.end_statement()?;
        Ok(Some(CfgEntry::Property(CfgProperty {
            name: word,
            value,
            append,
        })))
    }

    /// `{ A, B = 5, C }`: values without `=` count up from the previous one.
    fn enum_values(&mut self) -> Result<(), ParseError> {
        self.expect(b'{')?;
        let mut next = 0_i32;
        loop {
            self.ws();
            if self.eat(b'}') {
                return Ok(());
            }
            let name = self.expect_name("an enum name")?;
            self.ws();
            if self.eat(b'=') {
                self.ws();
                let start = self.i;
                let raw = self.raw_until(b",}").trim();
                let Some(CfgValue::Long(value)) = number(raw) else {
                    self.i = start;
                    return Err(self.err(format!("expected a number, found {}", self.found())));
                };
                next = value;
            }
            self.enums.push(CfgEnumValue { name, value: next });
            next = next.wrapping_add(1);
            self.ws();
            if !self.eat(b',') && self.peek() != Some(b'}') {
                return Err(self.err(format!("expected `,` or `}}`, found {}", self.found())));
            }
        }
    }

    fn value(&mut self) -> Result<CfgValue, ParseError> {
        let start = self.i;
        if matches!(self.peek(), Some(b'"' | b'\'')) {
            let text = self.strings()?;
            // A quoted string followed by more text is all text: `'Indep' >> "faction"`
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\r')) {
                self.i += 1;
            }
            if matches!(self.peek(), Some(b';' | b'}' | b'\n') | None) {
                return Ok(CfgValue::String(text));
            }
            self.i = start;
        }
        let value = unquoted(self.raw_until(b";}\n").trim());
        // `name = ;` is an empty string
        if value == CfgValue::String(String::new()) && self.peek() != Some(b';') {
            self.i = start;
            return Err(self.err(format!("expected a value, found {}", self.found())));
        }
        Ok(value)
    }

    fn element(&mut self) -> Result<CfgValue, ParseError> {
        let start = self.i;
        if matches!(self.peek(), Some(b'"' | b'\'')) {
            let text = self.strings()?;
            self.ws();
            if matches!(self.peek(), Some(b',' | b';' | b'}')) {
                return Ok(CfgValue::String(text));
            }
            self.i = start;
        }
        let raw = self.raw_until(b",};{").trim();
        if raw.is_empty() {
            return Err(self.err(format!("expected a value, found {}", self.found())));
        }
        Ok(unquoted(raw))
    }

    fn array(&mut self) -> Result<CfgValue, ParseError> {
        self.expect(b'{')?;
        let mut values = Vec::new();
        loop {
            self.ws();
            if self.eat(b'}') {
                return Ok(CfgValue::Array(values));
            }
            let value = if self.peek() == Some(b'{') {
                self.array()?
            } else {
                self.element()?
            };
            values.push(value);
            self.ws();
            // `;` instead of `,` is a typo the game accepts
            if !self.eat(b',') && !self.eat(b';') && self.peek() != Some(b'}') {
                return Err(self.err(format!("expected `,` or `}}`, found {}", self.found())));
            }
        }
    }

    /// A quoted string, or several joined by ` \n `, which become a line break.
    fn strings(&mut self) -> Result<String, ParseError> {
        let mut text = self.string()?;
        loop {
            let before = self.i;
            self.ws();
            if self.bytes[self.i..].starts_with(b"\\n") {
                self.i += 2;
                self.ws();
                if matches!(self.peek(), Some(b'"' | b'\'')) {
                    text.push('\n');
                    text.push_str(&self.string()?);
                    continue;
                }
            }
            self.i = before;
            return Ok(text);
        }
    }

    /// `"..."` or `'...'`, a doubled quote inside is an escaped quote.
    fn string(&mut self) -> Result<String, ParseError> {
        let start = self.i;
        let quote = self.bytes[self.i];
        self.i += 1;
        let mut text = String::new();
        loop {
            let Some(len) = self.bytes[self.i..].iter().position(|&b| b == quote) else {
                self.i = start;
                return Err(self.err("unterminated string"));
            };
            text.push_str(&self.src[self.i..self.i + len]);
            self.i += len + 1;
            if self.peek() == Some(quote) {
                text.push(quote as char);
                self.i += 1;
            } else {
                return Ok(text);
            }
        }
    }

    /// Text up to one of `stops` outside of parentheses and quotes. With no stops, reads one
    /// balanced `(...)` group. Quotes only count when they close on the same line, so an
    /// apostrophe in unquoted text doesn't swallow the rest of the file.
    fn raw_until(&mut self, stops: &[u8]) -> &str {
        let start = self.i;
        let mut depth = 0usize;
        while let Some(b) = self.peek() {
            match b {
                b'"' | b'\'' => {
                    let rest = &self.bytes[self.i + 1..];
                    let close = rest.iter().position(|&c| c == b || c == b'\n');
                    if let Some(p) = close.filter(|&p| rest[p] == b) {
                        self.i += p + 2;
                        continue;
                    }
                }
                b'(' => depth += 1,
                b')' if depth > 0 => {
                    depth -= 1;
                    if depth == 0 && stops.is_empty() {
                        self.i += 1;
                        break;
                    }
                }
                _ if depth == 0 && stops.contains(&b) => break,
                _ => {}
            }
            self.i += 1;
        }
        &self.src[start..self.i]
    }
}

/// An unquoted value: a number if it is one, otherwise the text as a string.
fn unquoted(raw: &str) -> CfgValue {
    number(raw).unwrap_or_else(|| CfgValue::String(raw.to_owned()))
}

fn number(raw: &str) -> Option<CfgValue> {
    let (negative, body) = match raw.as_bytes().first()? {
        b'-' => (true, &raw[1..]),
        b'+' => (false, &raw[1..]),
        _ => (false, raw),
    };
    let first = *body.as_bytes().first()?;
    if !(first.is_ascii_digit() || first == b'.') {
        return None;
    }

    let hex = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X"));
    let int = match hex {
        Some(hex) => i64::from_str_radix(hex, 16).ok(),
        None if body.bytes().all(|b| b.is_ascii_digit()) => body.parse::<i64>().ok(),
        None => None,
    };
    if let Some(int) = int {
        let int = if negative { -int } else { int };
        return Some(i32::try_from(int).map_or(CfgValue::Int64(int), CfgValue::Long));
    }

    if !body
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        return None;
    }
    let float = raw.parse::<f32>().ok()?;
    float.is_finite().then_some(CfgValue::Float(float))
}

fn report(src: &str, err: &ParseError) -> String {
    let pos = src[..err.pos.min(src.len())].chars().count();
    let report = Report::build(ReportKind::Error, (), pos)
        .with_message(&err.msg)
        .with_label(
            Label::new(pos..pos + 1)
                .with_message(err.msg.as_str().fg(Color::Red))
                .with_color(Color::Red),
        )
        .finish();
    let mut buf = Vec::new();
    if report
        .write(Source::from(src), Cursor::new(&mut buf))
        .is_ok()
    {
        String::from_utf8(buf).unwrap_or_default()
    } else {
        err.msg.clone()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn value(src: &str) -> CfgValue {
        match parse(&format!("x = {src};")).unwrap().0.remove(0) {
            CfgEntry::Property(p) => p.value,
            e => panic!("{e:?}"),
        }
    }

    fn array(src: &str) -> Vec<CfgValue> {
        match parse(&format!("x[] = {src};")).unwrap().0.remove(0) {
            CfgEntry::Property(CfgProperty {
                value: CfgValue::Array(a),
                ..
            }) => a,
            e => panic!("{e:?}"),
        }
    }

    fn s(text: &str) -> CfgValue {
        CfgValue::String(text.to_owned())
    }

    #[test]
    fn numbers() {
        assert_eq!(value("1"), CfgValue::Long(1));
        assert_eq!(value("-17"), CfgValue::Long(-17));
        assert_eq!(value("000000"), CfgValue::Long(0));
        assert_eq!(value("0x10"), CfgValue::Long(16));
        assert_eq!(value("4294967296"), CfgValue::Int64(4_294_967_296));
        assert_eq!(value("1.0"), CfgValue::Float(1.0));
        assert_eq!(value("-09.34"), CfgValue::Float(-9.34));
        assert_eq!(value(".5"), CfgValue::Float(0.5));
        assert_eq!(value("1e-3"), CfgValue::Float(0.001));
        assert_eq!(value("1.5E+2"), CfgValue::Float(150.0));
    }

    #[test]
    fn unquoted_values() {
        assert_eq!(value("true"), s("true"));
        assert_eq!(
            value("$STR_A3_Bohemia_Interactive"),
            s("$STR_A3_Bohemia_Interactive")
        );
        assert_eq!(value("0.2*4/3"), s("0.2*4/3"));
        assert_eq!(value("6 - 1"), s("6 - 1"));
        assert_eq!(value("(-0.01)"), s("(-0.01)"));
        assert_eq!(value("A_in.Stratis"), s("A_in.Stratis"));
        assert_eq!(value("1.2.3"), s("1.2.3"));
        assert_eq!(value("inf"), s("inf"));
        assert_eq!(
            value("__EVAL(safeZoneW - (5 * ((1 / (getResolution select 2)) * 1.25 * 4)))"),
            s("__EVAL(safeZoneW - (5 * ((1 / (getResolution select 2)) * 1.25 * 4)))")
        );
        assert_eq!(
            value("__EVAL(call compile \"1;2\")"),
            s("__EVAL(call compile \"1;2\")")
        );
    }

    #[test]
    fn strings() {
        assert_eq!(value("\"a \"\"b\"\" c\""), s("a \"b\" c"));
        assert_eq!(value("'a ''b'' c'"), s("a 'b' c"));
        assert_eq!(value("\"a\" \\n \"b\""), s("a\nb"));
        assert_eq!(value("\"multi\nline\""), s("multi\nline"));
        assert_eq!(value("\"#FF0000\""), s("#FF0000"));
    }

    #[test]
    fn arrays() {
        assert_eq!(array("{}"), vec![]);
        assert_eq!(
            array("{1, 2.5, \"a\",}"),
            vec![CfgValue::Long(1), CfgValue::Float(2.5), s("a")]
        );
        assert_eq!(array("{hour}"), vec![s("hour")]);
        assert_eq!(array("{%Dist}"), vec![s("%Dist")]);
        assert_eq!(
            array("{0.33333/1, 10/867}"),
            vec![s("0.33333/1"), s("10/867")]
        );
        assert_eq!(
            array("{{1,2},\n {3}}"),
            vec![
                CfgValue::Array(vec![CfgValue::Long(1), CfgValue::Long(2)]),
                CfgValue::Array(vec![CfgValue::Long(3)])
            ]
        );
        assert_eq!(array("{f(1, 2), 3}"), vec![s("f(1, 2)"), CfgValue::Long(3)]);
    }

    #[test]
    fn entries() {
        let cfg = parse(
            "class 30Rnd_mag; class true; class Delete {}; delete Old;\n\
             class A: 30Rnd_mag { items[] += {\"x\"}; class = 1; delete = 2; };\n\
             __EXEC(a = 1;);\nlast = 1",
        )
        .unwrap()
        .0;
        assert_eq!(cfg[0], CfgEntry::Extern("30Rnd_mag".to_owned()));
        assert_eq!(cfg[1], CfgEntry::Extern("true".to_owned()));
        assert!(
            matches!(&cfg[2], CfgEntry::Class(c) if c.name == "Delete" && c.entries.is_empty())
        );
        assert_eq!(cfg[3], CfgEntry::Delete("Old".to_owned()));
        let CfgEntry::Class(a) = &cfg[4] else {
            panic!()
        };
        assert_eq!(a.parent.as_deref(), Some("30Rnd_mag"));
        assert_eq!(
            a.entries[0],
            CfgEntry::Property(CfgProperty {
                name: "items".to_owned(),
                value: CfgValue::Array(vec![s("x")]),
                append: true,
            })
        );
        assert!(matches!(&a.entries[1], CfgEntry::Property(p) if p.name == "class"));
        assert!(matches!(&a.entries[2], CfgEntry::Property(p) if p.name == "delete"));
        assert_eq!(cfg.len(), 6);
        // Missing `;` before `}` and after classes is fine, `;` in arrays too
        assert!(parse("class A { x = 1 } class B {} x = 2").is_ok());
        assert_eq!(
            array("{{1, 2};\n}"),
            vec![CfgValue::Array(vec![CfgValue::Long(1), CfgValue::Long(2)])]
        );
    }

    #[test]
    fn errors() {
        let msg = |src: &str| match parse(src) {
            Err(AffError::ParseError(msg)) => msg,
            other => panic!("{other:?}"),
        };
        assert!(msg("class A {").contains("expected `}`, found end of file"));
        assert!(msg("class A { x = }").contains("expected a value"));
        assert!(msg("x = \"a").contains("unterminated string"));
        assert!(msg("x[] = {1, 2").contains("expected `,` or `}`, found end of file"));
        assert!(msg("x 1;").contains("expected `=`"));
        assert!(msg("class A: {};").contains("expected a parent class name"));
        assert!(msg("enum { A = x };").contains("expected a number"));
        assert!(msg("enum { A B };").contains("expected `,` or `}`, found `B`"));
        assert!(msg("class A: B x").contains("expected `{`, found `x`"));
        assert!(msg("delete A B;").contains("expected `;`, found `B`"));
        assert!(msg("= 1;").contains("expected a class or property, found `=`"));
        assert!(msg("x[] = {1, ,};").contains("expected a value, found `,`"));
        assert_eq!(value("+5"), CfgValue::Long(5));
        assert_eq!(value("f(a; b)"), s("f(a; b)"));
    }

    #[test]
    fn lenient() {
        // Missing `;` at line ends, before `}` and after classes, `;` in arrays, empty values,
        // a stray `}` at the end
        let (cfg, _) = parse(
            "class A { x = 1 } class B {} x = \"a\"\ny = 2\nz = ;\n\
             class C {\n w = 1;\n};\\\n};",
        )
        .unwrap();
        assert_eq!(cfg.len(), 6);
        assert!(matches!(&cfg[4], CfgEntry::Property(p) if p.value == s("")));
        assert_eq!(
            array("{{1, 2};\n}"),
            vec![CfgValue::Array(vec![CfgValue::Long(1), CfgValue::Long(2)])]
        );
        // Quoted strings followed by more text
        assert_eq!(value("\"a\" b"), s("\"a\" b"));
        assert_eq!(
            array("{'Indep' >> \"IND_C_F\", \"x\"}"),
            vec![s("'Indep' >> \"IND_C_F\""), s("x")]
        );
        assert_eq!(value("Don't stop"), s("Don't stop"));
    }

    #[test]
    fn enums() {
        let (entries, enums) =
            parse("enum { A, B = 5, C };\nenum {\n d = 0x10,\n e = -1\n}\nx = 1;").unwrap();
        assert_eq!(entries.len(), 1);
        let enums: Vec<(&str, i32)> = enums.iter().map(|e| (e.name.as_str(), e.value)).collect();
        assert_eq!(enums, [("A", 0), ("B", 5), ("C", 6), ("d", 16), ("e", -1)]);
    }
}
