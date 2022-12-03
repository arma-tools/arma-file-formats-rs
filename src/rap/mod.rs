pub(crate) mod parser {
    use pest_derive::Parser;

    #[derive(Parser)]
    #[grammar = "rap/cfg.pest"]
    pub struct RapConfigParser;
}

mod class;
mod config;
mod entry;
mod pretty_print;
mod property;
mod value;

use thiserror::Error;

pub use self::config::Cfg;
pub use self::pretty_print::PrettyPrint;
pub use self::{entry::CfgEntry, value::CfgValue};

#[derive(Debug)]
pub enum EntryReturn {
    Entry(CfgEntry),
    Value(CfgValue),
}

#[derive(Error, Debug)]
pub enum EntryError {
    #[error("Entry not found")]
    NotFound,

    #[error("unknown decoding error")]
    Unknown,
}
