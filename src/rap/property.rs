use std::io::{BufRead, Seek};

use pest::iterators::Pairs;

use super::{parser, pretty_print::PrettyPrint, value::CfgValue};
use crate::{core::read::ReadExtTrait, errors::RvffConfigError, rap::parser::Rule};

#[derive(Debug, Clone)]
pub struct CfgProperty {
    pub name: String,
    pub value: CfgValue,
}

impl CfgProperty {
    pub fn parse_property(token_rules: &mut Pairs<parser::Rule>) -> CfgProperty {
        let ident = token_rules.next().unwrap().as_str();
        let valr = token_rules.next().unwrap();
        let value = valr.as_str();
        match valr.as_rule() {
            Rule::string => CfgProperty {
                name: ident.to_owned(),
                value: CfgValue::String(value.to_owned()),
            },
            Rule::number => {
                let name = ident.to_owned();
                let num: f32 = value.parse().unwrap();
                if num.fract() != 0.0 {
                    CfgProperty {
                        name,
                        value: CfgValue::Float(num),
                    }
                } else {
                    CfgProperty {
                        name,
                        value: CfgValue::Long(num as i32),
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn parse_property_array(token_rules: &mut Pairs<Rule>) -> CfgProperty {
        let ident = token_rules.next().unwrap().as_str();
        let elmen = token_rules
            .next()
            .unwrap()
            .into_inner()
            .map(|r| {
                let value = r.as_str();
                match r.as_rule() {
                    Rule::string => CfgValue::String(value.to_owned()),
                    Rule::number => {
                        let num: f32 = value.parse().unwrap();
                        if num.fract() != 0.0 {
                            CfgValue::Float(num)
                        } else {
                            CfgValue::Long(num as i32)
                        }
                    }
                    _ => unreachable!(),
                }
            })
            .collect();

        CfgProperty {
            name: ident.to_owned(),
            value: CfgValue::Array(elmen),
        }
    }

    pub fn read_property<I>(reader: &mut I, is_array: bool) -> Result<CfgProperty, RvffConfigError>
    where
        I: BufRead + Seek,
    {
        if is_array {
            let name = reader.read_string_zt()?;
            Ok(CfgProperty {
                name,
                value: CfgValue::read_array(reader)?,
            })
        } else {
            let typ_id = reader.read_u8()?;
            let name = reader.read_string_zt()?;
            Ok(CfgProperty {
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
            println!("{}{}[] = {};", indent, self.name, self.value.to_strr());
        } else {
            println!("{}{} = {};", indent, self.name, self.value.to_strr());
        }
    }
}
