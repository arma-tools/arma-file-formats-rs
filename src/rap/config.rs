use std::io::{BufRead, Cursor, Seek};

use super::{entry::CfgEntry, parser::parse, pretty_print::PrettyPrint, EntryReturn};
use crate::{
    core::{decompress_lzss_unk_size, read::ReadExtTrait},
    errors::RvffError,
};

const RAP_MAGIC: u32 = 1_348_563_456;
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Cfg {
    pub enum_offset: u32,
    pub inherited_classname: String,
    pub entries: Vec<CfgEntry>,
}
impl Cfg {
    pub fn is_valid_rap_bin<I>(reader: &mut I) -> bool
    where
        I: BufRead + Seek,
    {
        matches!(reader.read_u32(), Ok(v) if v == RAP_MAGIC)
            && matches!(reader.read_u32(), Ok(v) if v == 0)
            && matches!(reader.read_u32(), Ok(v) if v == 8)
    }

    pub fn read_config<I>(reader: &mut I) -> Result<Self, RvffError>
    where
        I: BufRead + Seek,
    {
        if !Self::is_valid_rap_bin(reader) {
            return Err(RvffError::InvalidFileError);
        }

        let enum_offset = reader.read_u32()?;
        let inherited_classname = reader.read_string_zt()?;

        let entry_count = reader.read_compressed_int()?;

        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let entry = CfgEntry::parse_entry(reader)?;
            entries.push(entry);
        }

        Ok(Self {
            enum_offset,
            inherited_classname,
            entries,
        })
    }

    pub fn read_data(data: &[u8]) -> Result<Self, RvffError> {
        let mut reader = Cursor::new(data);
        Self::read(&mut reader)
    }

    pub fn read<I>(reader: &mut I) -> Result<Self, RvffError>
    where
        I: BufRead + Seek,
    {
        let is_valid_bin = Self::is_valid_rap_bin(reader);
        reader.rewind()?;
        if is_valid_bin {
            return Self::read_config(reader);
        }

        reader.read_u8()?;
        if matches!(reader.read_u32(), Ok(v) if v == RAP_MAGIC) {
            if let Ok(data) = decompress_lzss_unk_size(reader) {
                let mut reader = Cursor::new(data);
                let is_valid_bin = Self::is_valid_rap_bin(&mut reader);
                reader.rewind()?;
                if is_valid_bin {
                    return Self::read_config(&mut reader);
                }
            }
        }

        let mut cfg_text = String::new();
        reader.rewind()?;
        if let Err(err) = reader.read_to_string(&mut cfg_text) {
            // try lzss decompression
            reader.rewind()?;
            if let Ok(uncomp_data) = decompress_lzss_unk_size(reader) {
                if let Ok(cfg) = String::from_utf8(uncomp_data) {
                    if let Ok(entries) = parse(&cfg) {
                        return Ok(Self {
                            enum_offset: 0,
                            inherited_classname: String::new(),
                            entries,
                        });
                    }
                }
            }
            return Err(err.into());
        }

        Self::parse_config(&cfg_text)
    }

    pub fn parse_config(cfg: &str) -> Result<Self, RvffError> {
        let entries = parse(cfg)?;
        Ok(Self {
            enum_offset: 0,
            inherited_classname: String::new(),
            entries,
        })
    }

    #[must_use]
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
        for e in &self.entries {
            e.pretty_print(indentation_count);
        }
    }
}
