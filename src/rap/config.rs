use std::io::{BufRead, Seek};

use no_comment::{languages, IntoWithoutComments};
use pest::Parser;

use super::{entry::CfgEntry, parser::RapConfigParser, pretty_print::PrettyPrint, EntryReturn};
use crate::{
    core::read::ReadExtTrait,
    errors::{RvffConfigError, RvffConfigErrorKind},
    rap::parser::Rule,
};

const RAP_MAGIC: u32 = 1348563456;

#[derive(Debug)]
pub struct Cfg {
    #[allow(dead_code)]
    enum_offset: u32,
    pub inherited_classname: String,
    pub entries: Vec<CfgEntry>,
}
impl Cfg {
    pub fn parse_config(file_content: &str) -> Result<Cfg, RvffConfigError> {
        //Box<Error<Rule>>
        let file_content: String = file_content
            .chars()
            .without_comments(languages::c())
            .collect();
        match RapConfigParser::parse(Rule::cfg, &file_content) {
            Ok(rules) => {
                let mut entries = Vec::new();
                for r in rules {
                    if r.as_rule() != Rule::EOI {
                        entries.push(CfgEntry::parse_value(r));
                    }
                }
                Ok(Cfg {
                    entries,
                    enum_offset: 0,
                    inherited_classname: String::new(),
                })
            }
            Err(err) => Err(RvffConfigError::from(RvffConfigErrorKind::RvffPestError(
                format!("{}", err),
            ))),
        }
    }

    pub fn is_valid_rap_bin<I>(reader: &mut I) -> bool
    where
        I: BufRead + Seek,
    {
        matches!(reader.read_u32(), Ok(v) if v == RAP_MAGIC)
            && matches!(reader.read_u32(), Ok(v) if v == 0)
            && matches!(reader.read_u32(), Ok(v) if v == 8)
    }

    pub fn read_config<I>(reader: &mut I) -> Result<Cfg, RvffConfigError>
    where
        I: BufRead + Seek,
    {
        if !Cfg::is_valid_rap_bin(reader) {
            return Err(RvffConfigErrorKind::InvalidFileError.into());
        }

        let enum_offset = reader.read_u32()?;
        let inherited_classname = reader.read_string_zt()?;

        let entry_count = reader.read_compressed_int()?;

        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let entry = CfgEntry::parse_entry(reader)?;
            entries.push(entry);
        }

        Ok(Cfg {
            enum_offset,
            inherited_classname,
            entries,
        })
    }

    pub fn get_entry(&self, path: &[&str]) -> Option<EntryReturn> {
        for entry in &self.entries {
            if let Some(entry_found) = entry.get_entry(path) {
                return Some(entry_found);
            }
        }
        None
    }
}

impl PrettyPrint for Cfg {
    fn pretty_print(&self, indentation_count: u32) {
        for e in self.entries.iter() {
            e.pretty_print(indentation_count);
        }
    }
}
