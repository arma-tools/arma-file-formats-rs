use std::io::{Read, Seek};

use crate::{core::read::ReadExtTrait, errors::AffError};

use super::{class::CfgClass, pretty_print::PrettyPrint, property::CfgProperty, EntryReturn};

#[derive(Debug, PartialEq, Clone)]
pub enum CfgEntry {
    Property(CfgProperty),
    Class(CfgClass),
    Extern(String),
    Delete(String),
}

impl CfgEntry {
    pub fn parse_entry<I>(reader: &mut I) -> Result<Self, AffError>
    where
        I: Read + Seek,
    {
        let typ_id = reader.read_u8()?;
        Ok(match typ_id {
            0 => Self::Class(CfgClass::read_class(reader)?),
            1 => Self::Property(CfgProperty::read_property(reader, false)?),
            2 => Self::Property(CfgProperty::read_property(reader, true)?),
            3 => Self::Extern(reader.read_string_zt()?),
            4 => Self::Delete(reader.read_string_zt()?),
            _ => panic!("Unknown typ id: {typ_id}"),
        })
    }
    #[must_use]
    pub fn get_entry(&self, path: &[&str]) -> Option<EntryReturn> {
        let Some(first) = path.first() else {
            return None;
        };

        let cur = *first;

        let last = path.len() == 1;
        match self {
            Self::Property(prop) => {
                if last && prop.name == cur {
                    return Some(EntryReturn::Value(prop.value.clone()));
                }
            }
            Self::Class(class) => {
                if last && class.name == cur {
                    return Some(EntryReturn::Entry(Self::Class(class.clone())));
                } else if class.name == cur {
                    return class.get_entry(&path[1..path.len()]);
                }
            }
            Self::Extern(ext) => {
                if last && ext == cur {
                    return Some(EntryReturn::Entry(Self::Extern(ext.to_string())));
                }
            }
            Self::Delete(del) => {
                if last && del == cur {
                    return Some(EntryReturn::Entry(Self::Delete(del.to_string())));
                }
            }
        }

        None
    }

    #[must_use]
    pub fn as_property(&self) -> Option<CfgProperty> {
        if let Self::Property(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_class(&self) -> Option<CfgClass> {
        if let Self::Class(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_extern(&self) -> Option<String> {
        if let Self::Extern(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_delete(&self) -> Option<String> {
        if let Self::Delete(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }
}

impl PrettyPrint for CfgEntry {
    fn pretty_print(&self, indentation_count: u32) {
        match self {
            Self::Property(pair) => pair.pretty_print(indentation_count),
            Self::Class(class) => class.pretty_print(indentation_count),
            Self::Extern(extern_name) => {
                let indent = (0..indentation_count).map(|_| " ").collect::<String>();
                println!("{indent}class {extern_name};");
            }
            Self::Delete(deleted_name) => {
                let indent = (0..indentation_count).map(|_| " ").collect::<String>();
                println!("{indent}delete {deleted_name};");
            }
        }
    }
}
