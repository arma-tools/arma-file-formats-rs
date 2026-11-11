use std::io::{Read, Seek};

use crate::{
    core::read::ReadExtTrait,
    errors::{AffError, RapError},
};

#[derive(Debug, PartialEq, Clone)]
pub enum CfgValue {
    Float(f32),
    Long(i32),
    /// Integers that don't fit an `i32`
    Int64(i64),
    String(String),
    Array(Vec<Self>),
}

impl CfgValue {
    pub fn read_value<I>(reader: &mut I, typ_id: Option<u8>) -> Result<Self, AffError>
    where
        I: Read + Seek,
    {
        let typ_id = if let Some(typ_id) = typ_id {
            typ_id
        } else {
            reader.read_u8()?
        };

        Ok(match typ_id {
            // 4 is an expression, `__EVAL` for example
            0 | 4 => Self::String(reader.read_string_zt()?),
            1 => Self::Float(reader.read_f32()?),
            2 => Self::Long(reader.read_i32()?),
            3 => Self::read_array(reader)?,
            6 => {
                let low = u64::from(reader.read_u32()?);
                let high = u64::from(reader.read_u32()?);
                Self::Int64((high << 32 | low) as i64)
            }
            _ => return Err(RapError::UnknownValueType(typ_id).into()),
        })
    }

    pub fn read_array<I>(reader: &mut I) -> Result<Self, AffError>
    where
        I: Read + Seek,
    {
        let entry_count = reader.read_compressed_int()?;
        // The count of a damaged file can be anything, don't allocate it up front
        let mut entries = Vec::with_capacity((entry_count as usize).min(1024));
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

    /// `Long` and `Int64` values
    #[must_use]
    pub const fn as_int64(&self) -> Option<i64> {
        match self {
            Self::Long(val) => Some(*val as i64),
            Self::Int64(val) => Some(*val),
            _ => None,
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
            Self::Int64(num) => num.to_string(),
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
