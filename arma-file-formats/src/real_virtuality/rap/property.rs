use std::io::{Read, Seek};

use super::value::CfgValue;
use crate::{
    core::read::ReadExtTrait,
    errors::{AffError, RapError},
};

#[derive(Debug, PartialEq, Clone)]
pub struct CfgProperty {
    pub name: String,
    pub value: CfgValue,
    /// `name[] += {...}`: the values are added to the array inherited from the parent class
    pub append: bool,
}

impl CfgProperty {
    pub fn read_property<I>(reader: &mut I, is_array: bool) -> Result<Self, AffError>
    where
        I: Read + Seek,
    {
        if is_array {
            let name = reader.read_string_zt()?;
            Ok(Self {
                name,
                value: CfgValue::read_array(reader)?,
                append: false,
            })
        } else {
            let typ_id = reader.read_u8()?;
            let name = reader.read_string_zt()?;
            Ok(Self {
                name,
                value: CfgValue::read_value(reader, Some(typ_id))?,
                append: false,
            })
        }
    }

    /// Entry type 5, an array with flags, 1 is `+=`.
    pub(crate) fn read_array_extension<I>(reader: &mut I) -> Result<Self, AffError>
    where
        I: Read + Seek,
    {
        let flags = reader.read_u32()?;
        if flags != 1 {
            return Err(RapError::UnknownArrayFlags(flags).into());
        }
        let name = reader.read_string_zt()?;
        Ok(Self {
            name,
            value: CfgValue::read_array(reader)?,
            append: true,
        })
    }
}
