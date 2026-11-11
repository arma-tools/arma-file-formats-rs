use std::io::{Cursor, Read, Seek, SeekFrom};

use super::{
    EntryReturn, entry::CfgEntry, parser::parse, preprocessor::Preprocessor,
    pretty_print::PrettyPrint,
};
use crate::{
    core::{decompress_lzss_unk_size, read::ReadExtTrait},
    errors::AffError,
};

const RAP_MAGIC: u32 = 1_348_563_456;

/// A value of an `enum { name = value, ... };` block. The game's binarizer stores the names in
/// lowercase.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CfgEnumValue {
    pub name: String,
    pub value: i32,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Cfg {
    pub enum_offset: u32,
    pub inherited_classname: String,
    pub entries: Vec<CfgEntry>,
    /// All `enum` values of the config
    pub enums: Vec<CfgEnumValue>,
}
impl Cfg {
    pub fn is_valid_rap_bin<I>(reader: &mut I) -> bool
    where
        I: Read + Seek,
    {
        matches!(reader.read_u32(), Ok(v) if v == RAP_MAGIC)
            && matches!(reader.read_u32(), Ok(v) if v == 0)
            && matches!(reader.read_u32(), Ok(v) if v == 8)
    }

    pub fn read_config<I>(reader: &mut I) -> Result<Self, AffError>
    where
        I: Read + Seek,
    {
        if !Self::is_valid_rap_bin(reader) {
            return Err(AffError::InvalidFileError);
        }

        let enum_offset = reader.read_u32()?;
        let inherited_classname = reader.read_string_zt()?;

        let entry_count = reader.read_compressed_int()?;

        let mut entries = Vec::with_capacity((entry_count as usize).min(1024));
        for _ in 0..entry_count {
            let entry = CfgEntry::parse_entry(reader)?;
            entries.push(entry);
        }

        let enums = if enum_offset == 0 {
            Vec::new()
        } else {
            Self::read_enums(reader, enum_offset)
        };

        Ok(Self {
            enum_offset,
            inherited_classname,
            entries,
            enums,
        })
    }

    /// The enum table at the end of the file. Its count can be larger than the values stored
    /// (24 for 15 values in the game's 3den config.bin), the table ends with the file. Damaged
    /// files have offsets past the end: the values read until a read fails are returned, the
    /// entries of the config are fine without them.
    fn read_enums<I>(reader: &mut I, offset: u32) -> Vec<CfgEnumValue>
    where
        I: Read + Seek,
    {
        let mut enums = Vec::new();
        let read = |reader: &mut I, enums: &mut Vec<CfgEnumValue>| -> Result<(), AffError> {
            let len = reader.seek(SeekFrom::End(0))?;
            reader.seek(SeekFrom::Start(u64::from(offset)))?;
            let count = reader.read_u32()?;
            for _ in 0..count {
                if reader.stream_position()? >= len {
                    break;
                }
                let name = reader.read_string_zt()?;
                let value = reader.read_i32()?;
                enums.push(CfgEnumValue { name, value });
            }
            Ok(())
        };
        let _ = read(reader, &mut enums);
        enums
    }

    pub fn read_data(data: &[u8]) -> Result<Self, AffError> {
        let mut reader = Cursor::new(data);
        Self::read(&mut reader)
    }

    pub fn read<I>(reader: &mut I) -> Result<Self, AffError>
    where
        I: Read + Seek,
    {
        let is_valid_bin = Self::is_valid_rap_bin(reader);
        reader.rewind()?;
        if is_valid_bin {
            return Self::read_config(reader);
        }

        reader.read_u8()?;
        if matches!(reader.read_u32(), Ok(v) if v == RAP_MAGIC)
            && let Ok(data) = decompress_lzss_unk_size(reader)
        {
            let mut reader = Cursor::new(data);
            let is_valid_bin = Self::is_valid_rap_bin(&mut reader);
            reader.rewind()?;
            if is_valid_bin {
                return Self::read_config(&mut reader);
            }
        }

        let mut cfg_text = String::new();
        reader.rewind()?;
        if let Err(err) = reader.read_to_string(&mut cfg_text) {
            // try lzss decompression
            reader.rewind()?;
            if let Ok(uncomp_data) = decompress_lzss_unk_size(reader)
                && let Ok(cfg) = String::from_utf8(uncomp_data)
                && let Ok(config) = Self::parse_config(&cfg)
            {
                return Ok(config);
            }
            return Err(err.into());
        }

        Self::parse_config(&cfg_text)
    }

    /// Parses a text config. Runs the preprocessor first, without an include resolver,
    /// so `#include` fails, use [`Self::parse_config_with`] for configs with includes.
    pub fn parse_config(cfg: &str) -> Result<Self, AffError> {
        Self::parse_config_with(cfg, None, &mut Preprocessor::new())
    }

    /// Parses a text config after running it through `preprocessor`. `file` is the path
    /// of the config, used to resolve relative includes.
    pub fn parse_config_with(
        cfg: &str,
        file: Option<&str>,
        preprocessor: &mut Preprocessor<'_>,
    ) -> Result<Self, AffError> {
        let source = preprocessor.preprocess(cfg, file)?;
        Self::parse_preprocessed(&source)
    }

    /// Loads `path` through the include resolver of `preprocessor`, preprocesses and parses it.
    pub fn parse_path(path: &str, preprocessor: &mut Preprocessor<'_>) -> Result<Self, AffError> {
        let source = preprocessor.preprocess_path(path)?;
        Self::parse_preprocessed(&source)
    }

    fn parse_preprocessed(source: &str) -> Result<Self, AffError> {
        let (entries, enums) = parse(source)?;
        Ok(Self {
            enum_offset: 0,
            inherited_classname: String::new(),
            entries,
            enums,
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
