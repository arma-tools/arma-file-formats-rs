use std::io::{Read, Seek};

use crate::{core::read::ReadExtTrait, errors::AffError};

use super::{entry::CfgEntry, pretty_print::PrettyPrint, EntryReturn};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct CfgClass {
    pub name: String,
    pub parent: Option<String>,
    pub entries: Vec<CfgEntry>,
}

impl CfgClass {
    pub fn read_class<I>(reader: &mut I) -> Result<Self, AffError>
    where
        I: Read + Seek,
    {
        let name = reader.read_string_zt()?;
        let offset = u64::from(reader.read_u32()?);

        let pos = reader.stream_position()?;
        reader.seek(std::io::SeekFrom::Start(offset))?;

        let parent = reader.read_string_zt()?;

        let entry_count = reader.read_compressed_int()?;

        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            entries.push(CfgEntry::parse_entry(reader)?);
        }
        reader.seek(std::io::SeekFrom::Start(pos))?;

        Ok(Self {
            name,
            parent: if parent.is_empty() {
                None
            } else {
                Some(parent)
            },
            entries,
        })
    }

    #[must_use]
    pub fn get_entry(&self, path: &[&str]) -> Option<EntryReturn> {
        let Some(first) = path.first() else {
            return None;
        };

        let cur = *first;

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
            .map(|f| format!(": {f}"))
            .unwrap_or_default();
        println!("{indent}class {} {parent}", self.name);
        println!("{indent}{{");
        for entry in &self.entries {
            entry.pretty_print(indentation_count + 4);
        }
        println!("{indent}}};");
    }
}
