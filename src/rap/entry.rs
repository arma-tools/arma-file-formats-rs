use std::io::{BufRead, Seek};

use crate::{core::read::ReadExtTrait, errors::RvffError};

use super::{class::CfgClass, pretty_print::PrettyPrint, property::CfgProperty, EntryReturn};

#[derive(Debug, Clone)]
pub enum CfgEntry {
    Property(CfgProperty),
    Class(CfgClass),
    Extern(String),
    Delete(String),
}

impl CfgEntry {
    pub fn parse_entry<I>(reader: &mut I) -> Result<CfgEntry, RvffError>
    where
        I: BufRead + Seek,
    {
        let typ_id = reader.read_u8()?;
        Ok(match typ_id {
            0 => CfgEntry::Class(CfgClass::read_class(reader)?),
            1 => CfgEntry::Property(CfgProperty::read_property(reader, false)?),
            2 => CfgEntry::Property(CfgProperty::read_property(reader, true)?),
            3 => CfgEntry::Extern(reader.read_string_zt()?),
            4 => CfgEntry::Delete(reader.read_string_zt()?),
            _ => panic!("Unknown typ id: {}", typ_id),
        })
    }
    pub fn get_entry(&self, path: &[&str]) -> Option<EntryReturn> {
        let cur = *path.first().unwrap();
        let last = path.len() == 1;
        match self {
            CfgEntry::Property(prop) => {
                if last && prop.name == cur {
                    return Some(EntryReturn::Value(prop.value.clone()));
                }
            }
            CfgEntry::Class(class) => {
                if last && class.name == cur {
                    return Some(EntryReturn::Entry(CfgEntry::Class(class.clone())));
                } else if class.name == cur {
                    return class.get_entry(&path[1..path.len()]);
                }
            }
            CfgEntry::Extern(ext) => {
                if last && ext == cur {
                    return Some(EntryReturn::Entry(CfgEntry::Extern(ext.to_string())));
                }
            }
            CfgEntry::Delete(del) => {
                if last && del == cur {
                    return Some(EntryReturn::Entry(CfgEntry::Delete(del.to_string())));
                }
            }
        }

        None
    }

    pub fn as_property(&self) -> Option<CfgProperty> {
        if let CfgEntry::Property(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }

    pub fn as_class(&self) -> Option<CfgClass> {
        if let CfgEntry::Class(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }

    pub fn as_extern(&self) -> Option<String> {
        if let CfgEntry::Extern(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }

    pub fn as_delete(&self) -> Option<String> {
        if let CfgEntry::Delete(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }
}

impl PrettyPrint for CfgEntry {
    fn pretty_print(&self, indentation_count: u32) {
        match self {
            CfgEntry::Property(pair) => pair.pretty_print(indentation_count),
            CfgEntry::Class(class) => class.pretty_print(indentation_count),
            CfgEntry::Extern(extern_name) => {
                let indent = (0..indentation_count).map(|_| " ").collect::<String>();
                println!("{}class {};", indent, extern_name);
            }
            CfgEntry::Delete(deleted_name) => {
                let indent = (0..indentation_count).map(|_| " ").collect::<String>();
                println!("{}delete {};", indent, deleted_name);
            }
        }
    }
}
