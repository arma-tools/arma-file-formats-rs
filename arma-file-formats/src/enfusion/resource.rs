//! Enfusion resource references.
//!
//! Resources reference each other as `{GUID}path`, e.g.
//! `{3C10D749A4E84B38}Common/Materials/Default.emat`: the 64 bit resource GUID as 16 hex digits,
//! followed by the path inside the game data. Either part may be missing (`{GUID}` alone, or
//! a plain path). The GUID is the actual identity of a resource, the path is a hint that may be
//! outdated after a resource was moved. Game PAKs contain no `.meta` files to map GUIDs to
//! paths, so in practice resources are looked up by path, case insensitively.

use std::fmt;

/// A `{GUID}path` reference to another resource.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceRef {
    pub guid: Option<u64>,
    /// Path with `/` separators, may be empty.
    pub path: String,
}

impl ResourceRef {
    /// Parses `{GUID}path`, `{GUID}` or a plain `path`. Returns `None` for an empty string.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (guid, path) = text
            .strip_prefix('{')
            .and_then(|rest| rest.split_once('}'))
            .and_then(|(hex, path)| Some((u64::from_str_radix(hex, 16).ok()?, path)))
            .map_or((None, text), |(guid, path)| (Some(guid), path));
        if guid.is_none() && path.is_empty() {
            return None;
        }
        Some(Self {
            guid,
            path: path.replace('\\', "/"),
        })
    }

    /// The path in lowercase, for case insensitive lookups.
    #[must_use]
    pub fn lookup_key(&self) -> String {
        self.path.to_lowercase()
    }
}

impl fmt::Display for ResourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(guid) = self.guid {
            write!(f, "{{{guid:016X}}}")?;
        }
        f.write_str(&self.path)
    }
}
