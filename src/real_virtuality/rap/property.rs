use std::io::{BufRead, Seek};

use super::{pretty_print::PrettyPrint, value::CfgValue};
use crate::{errors::RvffError, real_virtuality::core::read::ReadExtTrait};

#[derive(Debug, PartialEq, Clone)]
pub struct CfgProperty {
    pub name: String,
    pub value: CfgValue,
}

impl CfgProperty {
    pub fn read_property<I>(reader: &mut I, is_array: bool) -> Result<Self, RvffError>
    where
        I: BufRead + Seek,
    {
        if is_array {
            let name = reader.read_string_zt()?;
            Ok(Self {
                name,
                value: CfgValue::read_array(reader)?,
            })
        } else {
            let typ_id = reader.read_u8()?;
            let name = reader.read_string_zt()?;
            Ok(Self {
                name,
                value: CfgValue::read_value(reader, Some(typ_id))?,
            })
        }
    }
}

impl PrettyPrint for CfgProperty {
    fn pretty_print(&self, indentation_count: u32) {
        let indent = (0..indentation_count).map(|_| " ").collect::<String>();
        if let CfgValue::Array(_) = self.value {
            println!("{indent}{}[] = {};", self.name, self.value.to_strr());
        } else {
            println!("{indent}{} = {};", self.name, self.value.to_strr());
        }
    }
}
