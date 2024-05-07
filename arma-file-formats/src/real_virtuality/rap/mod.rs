mod class;
mod config;
mod entry;
mod parser;
mod pretty_print;
mod property;
mod value;

use thiserror::Error;

pub use self::config::Cfg;
pub use self::pretty_print::PrettyPrint;
pub use self::{class::CfgClass, property::CfgProperty};
pub use self::{entry::CfgEntry, value::CfgValue};

#[derive(Debug, PartialEq, Clone)]
pub enum EntryReturn {
    Entry(CfgEntry),
    Value(CfgValue),
}

impl EntryReturn {
    #[must_use]
    pub fn as_entry(&self) -> Option<CfgEntry> {
        if let Self::Entry(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_value(&self) -> Option<CfgValue> {
        if let Self::Value(val) = self {
            Some(val.clone())
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_property(&self) -> Option<CfgProperty> {
        self.as_entry().and_then(|e| e.as_property())
    }

    #[must_use]
    pub fn as_class(&self) -> Option<CfgClass> {
        self.as_entry().and_then(|e| e.as_class())
    }

    #[must_use]
    pub fn as_extern(&self) -> Option<String> {
        self.as_entry().and_then(|e| e.as_extern())
    }

    #[must_use]
    pub fn as_delete(&self) -> Option<String> {
        self.as_entry().and_then(|e| e.as_delete())
    }

    #[must_use]
    pub fn as_float(&self) -> Option<f32> {
        self.as_value().and_then(|v| v.as_float()).or_else(|| {
            self.as_entry()
                .and_then(|e| e.as_property())
                .and_then(|p| p.value.as_float())
        })
    }

    #[must_use]
    pub fn as_long(&self) -> Option<i32> {
        self.as_value().and_then(|v| v.as_long()).or_else(|| {
            self.as_entry()
                .and_then(|e| e.as_property())
                .and_then(|p| p.value.as_long())
        })
    }

    #[must_use]
    pub fn as_string(&self) -> Option<String> {
        self.as_value().and_then(|v| v.as_string()).or_else(|| {
            self.as_entry()
                .and_then(|e| e.as_property())
                .and_then(|p| p.value.as_string())
        })
    }

    #[must_use]
    pub fn as_array(&self) -> Option<Vec<CfgValue>> {
        self.as_value().and_then(|v| v.as_array()).or_else(|| {
            self.as_entry()
                .and_then(|e| e.as_property())
                .and_then(|p| p.value.as_array())
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Error)]
pub enum EntryError {
    #[error("Entry not found")]
    NotFound,

    #[error("unknown decoding error")]
    Unknown,
}
