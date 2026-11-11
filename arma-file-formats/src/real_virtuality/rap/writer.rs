//! Config text that parses back to the same config: `cfg.to_string()`.
//!
//! Strings are quoted with `"` doubled, line breaks in them are written as ` \n `. Floats
//! always have a `.` or an exponent so they read back as floats.

use std::fmt::{self, Display, Formatter, Write};

use super::{
    CfgClass, CfgEntry, CfgProperty, CfgValue, PrettyPrint, config::Cfg, config::CfgEnumValue,
};

const INDENT: &str = "    ";

impl Display for CfgValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Float(num) => write!(f, "{num:?}"),
            Self::Long(num) => write!(f, "{num}"),
            Self::Int64(num) => write!(f, "{num}"),
            Self::String(text) => {
                for (i, line) in text.split('\n').enumerate() {
                    if i > 0 {
                        f.write_str(" \\n ")?;
                    }
                    write!(f, "\"{}\"", line.replace('"', "\"\""))?;
                }
                Ok(())
            }
            Self::Array(values) => {
                f.write_char('{')?;
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{value}")?;
                }
                f.write_char('}')
            }
        }
    }
}

fn write_property(f: &mut impl Write, property: &CfgProperty, indent: &str) -> fmt::Result {
    let name = &property.name;
    let value = &property.value;
    match (value, property.append) {
        (CfgValue::Array(_), true) => writeln!(f, "{indent}{name}[] += {value};"),
        (CfgValue::Array(_), false) => writeln!(f, "{indent}{name}[] = {value};"),
        _ => writeln!(f, "{indent}{name} = {value};"),
    }
}

fn write_class(f: &mut impl Write, class: &CfgClass, indent: &str) -> fmt::Result {
    let parent = class
        .parent
        .as_ref()
        .map(|parent| format!(": {parent}"))
        .unwrap_or_default();
    if class.entries.is_empty() {
        return writeln!(f, "{indent}class {}{parent} {{}};", class.name);
    }
    writeln!(f, "{indent}class {}{parent}", class.name)?;
    writeln!(f, "{indent}{{")?;
    let inner = format!("{indent}{INDENT}");
    for entry in &class.entries {
        write_entry(f, entry, &inner)?;
    }
    writeln!(f, "{indent}}};")
}

fn write_entry(f: &mut impl Write, entry: &CfgEntry, indent: &str) -> fmt::Result {
    match entry {
        CfgEntry::Property(property) => write_property(f, property, indent),
        CfgEntry::Class(class) => write_class(f, class, indent),
        CfgEntry::Extern(name) => writeln!(f, "{indent}class {name};"),
        CfgEntry::Delete(name) => writeln!(f, "{indent}delete {name};"),
    }
}

fn write_enums(f: &mut impl Write, enums: &[CfgEnumValue]) -> fmt::Result {
    if enums.is_empty() {
        return Ok(());
    }
    f.write_str("enum\n{\n")?;
    for (i, CfgEnumValue { name, value }) in enums.iter().enumerate() {
        let comma = if i + 1 < enums.len() { "," } else { "" };
        writeln!(f, "{INDENT}{name} = {value}{comma}")?;
    }
    f.write_str("};\n")
}

impl Display for CfgProperty {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_property(f, self, "")
    }
}

impl Display for CfgClass {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_class(f, self, "")
    }
}

impl Display for CfgEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_entry(f, self, "")
    }
}

impl Display for Cfg {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_enums(f, &self.enums)?;
        for entry in &self.entries {
            write_entry(f, entry, "")?;
        }
        Ok(())
    }
}

fn indented(indentation_count: u32, write: impl FnOnce(&mut String, &str) -> fmt::Result) {
    let indent = " ".repeat(indentation_count as usize);
    let mut text = String::new();
    if write(&mut text, &indent).is_ok() {
        print!("{text}");
    }
}

impl PrettyPrint for CfgProperty {
    fn pretty_print(&self, indentation_count: u32) {
        indented(indentation_count, |f, indent| {
            write_property(f, self, indent)
        });
    }
}

impl PrettyPrint for CfgClass {
    fn pretty_print(&self, indentation_count: u32) {
        indented(indentation_count, |f, indent| write_class(f, self, indent));
    }
}

impl PrettyPrint for CfgEntry {
    fn pretty_print(&self, indentation_count: u32) {
        indented(indentation_count, |f, indent| write_entry(f, self, indent));
    }
}

impl PrettyPrint for Cfg {
    fn pretty_print(&self, indentation_count: u32) {
        indented(indentation_count, |f, indent| {
            write_enums(f, &self.enums)?;
            self.entries
                .iter()
                .try_for_each(|entry| write_entry(f, entry, indent))
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn round_trip(text: &str) -> Cfg {
        let cfg = Cfg::parse_config(text).unwrap();
        let written = cfg.to_string();
        let again = Cfg::parse_config(&written).unwrap();
        assert_eq!(cfg, again, "{written}");
        cfg
    }

    #[test]
    fn values() {
        assert_eq!(CfgValue::Float(1.0).to_string(), "1.0");
        assert_eq!(CfgValue::Float(1e20).to_string(), "1e20");
        assert_eq!(CfgValue::Long(-3).to_string(), "-3");
        assert_eq!(CfgValue::Int64(1 << 40).to_string(), "1099511627776");
        assert_eq!(
            CfgValue::String("a \"b\"".to_owned()).to_string(),
            "\"a \"\"b\"\"\""
        );
        assert_eq!(
            CfgValue::String("a\nb".to_owned()).to_string(),
            "\"a\" \\n \"b\""
        );
        assert_eq!(
            CfgValue::Array(vec![CfgValue::Long(1), CfgValue::Array(vec![])]).to_string(),
            "{1, {}}"
        );
        assert_eq!(CfgValue::Long(1).to_strr(), "1");
    }

    #[test]
    fn config() {
        let cfg = round_trip(
            "enum { a = 1, b };\n\
             class Ext; delete Old;\n\
             class A: Ext { x = 1.0; y = \"q\"\"\" \\n \"line\"; items[] += {1, {2, \"3\"}}; class Empty {}; };\n\
             z[] = {};",
        );
        assert_eq!(
            cfg.to_string(),
            "enum\n{\n    a = 1,\n    b = 2\n};\n\
             class Ext;\n\
             delete Old;\n\
             class A: Ext\n{\n    x = 1.0;\n    y = \"q\"\"\" \\n \"line\";\n    items[] += {1, {2, \"3\"}};\n    class Empty {};\n};\n\
             z[] = {};\n"
        );
        let CfgEntry::Class(a) = &cfg.entries[2] else {
            panic!()
        };
        assert_eq!(a.to_string().lines().next(), Some("class A: Ext"));
        assert_eq!(a.entries[0].to_string(), "x = 1.0;\n");
        let CfgEntry::Property(items) = &a.entries[2] else {
            panic!()
        };
        assert_eq!(items.to_string(), "items[] += {1, {2, \"3\"}};\n");

        // Printing goes through the same writer
        cfg.pretty_print(2);
        a.pretty_print(0);
        a.entries[0].pretty_print(4);
        items.pretty_print(0);
    }
}
