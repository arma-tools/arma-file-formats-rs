use std::io::{BufRead, Seek};

use pest::iterators::Pairs;

use crate::{core::read::ReadExtTrait, errors::RvffConfigError, rap::parser::Rule};

use super::{entry::CfgEntry, pretty_print::PrettyPrint, EntryReturn};

#[derive(Debug, Clone)]
pub struct CfgClass {
    pub name: String,
    pub parent: Option<String>,
    pub entries: Vec<CfgEntry>,
}

impl CfgClass {
    pub fn parse_class(token_rules: &mut Pairs<Rule>) -> CfgClass {
        let name = token_rules.next().unwrap().as_str().to_owned();
        let mut parent_class: Option<String> = None;
        if let Some(snd_token) = token_rules.peek() {
            let rule = snd_token.as_rule();
            if rule == Rule::ident {
                parent_class = Some(snd_token.as_str().to_owned());
                token_rules.next();
            }
        } else {
            // empty class
            return CfgClass {
                name,
                parent: None,
                entries: Vec::new(),
            };
        }

        CfgClass {
            name,
            parent: parent_class,
            entries: token_rules.map(CfgEntry::parse_value).collect(),
        }
    }

    pub fn read_class<I>(reader: &mut I) -> Result<CfgClass, RvffConfigError>
    where
        I: BufRead + Seek,
    {
        let name = reader.read_string_zt()?;
        let offset = reader.read_u32()? as u64;

        let pos = reader.stream_position()?;
        reader.seek(std::io::SeekFrom::Start(offset))?;

        let parent = reader.read_string_zt()?;

        let entry_count = reader.read_compressed_int()?;

        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            entries.push(CfgEntry::parse_entry(reader)?);
        }
        reader.seek(std::io::SeekFrom::Start(pos))?;

        Ok(CfgClass {
            name,
            parent: if !parent.is_empty() {
                Some(parent)
            } else {
                None
            },
            entries,
        })
    }

    pub fn get_entry(&self, path: &[&str]) -> Option<EntryReturn> {
        let cur = *path.first().unwrap();
        let last = path.len() == 1;

        for entry in &self.entries {
            if last {
                match entry {
                    CfgEntry::Property(prop) => {
                        if prop.name == cur {
                            return Some(EntryReturn::Entry(CfgEntry::Property(prop.clone())));
                        }
                    }
                    CfgEntry::Class(class) => {
                        if class.name == cur {
                            return Some(EntryReturn::Entry(CfgEntry::Class(class.clone())));
                        }
                    }
                    CfgEntry::Extern(ext) => {
                        if ext == cur {
                            return Some(EntryReturn::Entry(CfgEntry::Extern(ext.clone())));
                        }
                    }
                    CfgEntry::Delete(del) => {
                        if del == cur {
                            return Some(EntryReturn::Entry(CfgEntry::Delete(del.clone())));
                        }
                    }
                }
            } else if let Some(entry_found) = entry.get_entry(path) {
                return Some(entry_found);
            }
        }

        None
    }
}

impl PrettyPrint for CfgClass {
    fn pretty_print(&self, indentation_count: u32) {
        let indent = (0..indentation_count).map(|_| " ").collect::<String>();
        let parent = self
            .parent
            .as_ref()
            .map(|f| format!(": {}", f))
            .unwrap_or_default();
        println!("{}class {} {}", indent, self.name, parent);
        println!("{}{{", indent);
        for entry in self.entries.iter() {
            entry.pretty_print(indentation_count + 4);
        }
        println!("{}}};", indent);
    }
}
