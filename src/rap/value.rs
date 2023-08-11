use std::io::{BufRead, Seek};

use crate::{core::read::ReadExtTrait, errors::RvffError};

#[derive(Debug, Clone)]
pub enum CfgValue {
    Float(f32),
    Long(i32),
    String(String),
    Array(Vec<CfgValue>),
}

impl CfgValue {
    pub fn read_value<I>(reader: &mut I, typ_id: Option<u8>) -> Result<Self, RvffError>
    where
        I: BufRead + Seek,
    {
        let typ_id = if let Some(typ_id) = typ_id {
            typ_id
        } else {
            reader.read_u8()?
        };

        Ok(match typ_id {
            0 | 4 => Self::String(reader.read_string_zt()?),
            1 => Self::Float(reader.read_f32()?),
            2 => Self::Long(reader.read_i32()?),
            3 => Self::read_array(reader)?,
            _ => panic!("Unknown typ id: {typ_id}"),
        })
    }

    pub fn read_array<I>(reader: &mut I) -> Result<Self, RvffError>
    where
        I: BufRead + Seek,
    {
        let entry_count = reader.read_compressed_int()?;
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let entry = Self::read_value(reader, None)?;
            entries.push(entry);
        }

        Ok(Self::Array(entries))
    }

    #[must_use]
    pub const fn as_float(&self) -> Option<f32> {
        if let Self::Float(val) = self {
            Some(*val)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn as_long(&self) -> Option<i32> {
        if let Self::Long(val) = self {
            Some(*val)
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_string(&self) -> Option<String> {
        if let Self::String(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<Vec<Self>> {
        if let Self::Array(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }
}

impl CfgValue {
    pub fn to_strr(&self) -> String {
        match self {
            Self::Float(num) => num.to_string(),
            Self::Long(num) => num.to_string(),
            Self::String(str) => format!("\"{}\"", str.trim_matches('"')),
            Self::Array(arr) => format!(
                "{{ {} }}",
                arr.iter()
                    .map(Self::to_strr)
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}
