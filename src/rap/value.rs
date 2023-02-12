use std::io::{BufRead, Seek};

use crate::{core::read::ReadExtTrait, errors::RvffConfigError};

#[derive(Debug, Clone)]
pub enum CfgValue {
    Float(f32),
    Long(i32),
    String(String),
    Array(Vec<CfgValue>),
}

impl CfgValue {
    pub fn read_value<I>(reader: &mut I, typ_id: Option<u8>) -> Result<CfgValue, RvffConfigError>
    where
        I: BufRead + Seek,
    {
        let typ_id = if let Some(typ_id) = typ_id {
            typ_id
        } else {
            reader.read_u8()?
        };

        Ok(match typ_id {
            0 => CfgValue::String(reader.read_string_zt()?),
            1 => CfgValue::Float(reader.read_f32()?),
            2 => CfgValue::Long(reader.read_i32()?),
            3 => CfgValue::read_array(reader)?,
            4 => CfgValue::String(reader.read_string_zt()?),
            _ => panic!("Unknown typ id: {}", typ_id),
        })
    }

    pub fn read_array<I>(reader: &mut I) -> Result<CfgValue, RvffConfigError>
    where
        I: BufRead + Seek,
    {
        let entry_count = reader.read_compressed_int()?;
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let entry = CfgValue::read_value(reader, None)?;
            entries.push(entry);
        }

        Ok(CfgValue::Array(entries))
    }

    pub fn as_float(&self) -> Option<f32> {
        if let CfgValue::Float(val) = self {
            Some(*val)
        } else {
            None
        }
    }

    pub fn as_long(&self) -> Option<i32> {
        if let CfgValue::Long(val) = self {
            Some(*val)
        } else {
            None
        }
    }

    pub fn as_string(&self) -> Option<String> {
        if let CfgValue::String(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }

    pub fn as_array(&self) -> Option<Vec<CfgValue>> {
        if let CfgValue::Array(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }
}

impl CfgValue {
    pub fn to_strr(&self) -> String {
        match self {
            CfgValue::Float(num) => num.to_string(),
            CfgValue::Long(num) => num.to_string(),
            CfgValue::String(str) => format!("\"{}\"", str.trim_matches('"')),
            CfgValue::Array(arr) => format!(
                "{{ {} }}",
                arr.iter()
                    .map(|x| x.to_strr())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}
