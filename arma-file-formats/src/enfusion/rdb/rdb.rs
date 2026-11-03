use std::{
    collections::HashMap,
    io::{Cursor, Read, Seek, Write},
};

use binrw::{BinWrite, io::NoSeek};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::{
    core::iff::FormHeader,
    enfusion::{pak::PakTimestamp, resource::ResourceRef},
    errors::{AffError, RdbError},
};

const RDB_FORM_TYPE: &[u8; 4] = b"RDBC";
/// The only known version.
pub const RDB_VERSION: u32 = 7;

/// Smallest possible entry: an empty path, flags, tag mask and GUID.
const MIN_ENTRY_SIZE: usize = 4 + 1 + 2 + 4 + 8;

/// Enfusion resource database (`resourceDatabase.rdb`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rdb {
    /// [`RDB_VERSION`].
    pub version: u32,
    /// The addon's Resource Browser tags (the `Tags` property of `.meta` files), in the order
    /// Workbench came across them. Bit `i` of [`RdbEntry::tags`] is tag `i`, so an addon has at
    /// most 32.
    pub tags: Vec<String>,
    /// Every directory and file of the addon, starting with the root directory (an empty path).
    /// Workbench sorts them by their lowercased path, so a directory comes right before its
    /// contents. It leaves out files with non-ASCII names.
    pub entries: Vec<RdbEntry>,
}

/// A directory or file of an [`Rdb`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdbEntry {
    /// Path from the addon root with `/` separators, without a leading `/`.
    pub path: String,
    /// [`RdbEntry::DIRECTORY`], [`RdbEntry::META`] and flags of unknown meaning.
    pub flags: u16,
    /// Bit mask of the entry's [`Rdb::tags`].
    pub tags: u32,
    /// The resource GUID from the `.meta`, `0` for a file without one (after a full scan, files
    /// Workbench noticed while it was running can have one). Directories without a
    /// `.meta` have one too: it depends only on the lowercased path (the same in every addon),
    /// the function is unknown.
    pub guid: u64,
    /// The `.meta` next to the entry, if it has one. Set exactly when the flags have
    /// [`RdbEntry::META`].
    pub meta: Option<RdbMeta>,
}

/// The `.meta` file of an [`RdbEntry`], at the time Workbench scanned it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RdbMeta {
    /// Modification time (UTC) of the `.meta` file.
    pub modified: PakTimestamp,
    /// Size of the `.meta` file in bytes.
    pub size: u32,
}

impl RdbEntry {
    /// The entry is a directory.
    pub const DIRECTORY: u16 = 0x01;
    /// A `.meta` (`<file>.meta`, or `<directory>.meta` next to a registered directory) belongs
    /// to the entry, [`RdbEntry::meta`] holds its time and size.
    pub const META: u16 = 0x02;
    /// Set on every entry Workbench writes.
    pub const UNKNOWN_04: u16 = 0x04;
    /// Seen only on the main string table (`Language/localization.st`) of the game's addons,
    /// not on other `.st` files, also not when the `.gproj` lists them as string tables.
    pub const UNKNOWN_10: u16 = 0x10;

    #[must_use]
    pub const fn is_directory(&self) -> bool {
        self.flags & Self::DIRECTORY != 0
    }

    /// The last component of the path, empty for the root directory.
    #[must_use]
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or_default()
    }

    /// A `{GUID}path` reference to the entry, without GUID if it has none.
    #[must_use]
    pub fn resource_ref(&self) -> ResourceRef {
        ResourceRef {
            guid: (self.guid != 0).then_some(self.guid),
            path: self.path.clone(),
        }
    }
}

impl Rdb {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let form = FormHeader::read_form(input)?;
        if &form.form_type != RDB_FORM_TYPE {
            return Err(RdbError::UnknownFormType(form.form_type_lossy()).into());
        }
        let body_size = u64::from(form.size.saturating_sub(4));
        let mut body = Vec::new();
        input.take(body_size).read_to_end(&mut body)?;
        if body.len() as u64 != body_size {
            return Err(RdbError::Truncated.into());
        }
        Ok(Self::parse(&body, form.size)?)
    }

    fn parse(body: &[u8], form_size: u32) -> Result<Self, RdbError> {
        let mut input = Cursor::new(body);
        let version = read_u32(&mut input)?;
        if version != RDB_VERSION {
            return Err(RdbError::UnsupportedVersion(version));
        }
        let file_size = read_u64(&mut input)?;
        let actual = u64::from(form_size) + 8;
        if file_size != actual {
            return Err(RdbError::SizeMismatch {
                header: file_size,
                actual,
            });
        }

        let count = read_u32(&mut input)?;
        let tags = (0..count)
            .map(|_| read_string(&mut input))
            .collect::<Result<_, _>>()?;

        let count = read_u32(&mut input)? as usize;
        // The count is untrusted, don't allocate more than the data can hold.
        let mut entries = Vec::with_capacity(count.min(body.len() / MIN_ENTRY_SIZE));
        for _ in 0..count {
            let path = read_string(&mut input)?;
            let flags = read_u16(&mut input)?;
            let tags = read_u32(&mut input)?;
            let guid = read_u64(&mut input)?;
            let meta = if flags & RdbEntry::META == 0 {
                None
            } else {
                Some(RdbMeta {
                    modified: PakTimestamp(read_u32(&mut input)?),
                    size: read_u32(&mut input)?,
                })
            };
            entries.push(RdbEntry {
                path,
                flags,
                tags,
                guid,
                meta,
            });
        }
        if input.position() != body.len() as u64 {
            return Err(RdbError::TrailingData);
        }

        Ok(Self {
            version,
            tags,
            entries,
        })
    }

    /// Writes the rdb at the current position of `out`. Entries are written in the given order.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let mut body = Vec::new();
        body.write_u32::<LittleEndian>(self.version)?;
        // The file size, filled in below.
        body.write_u64::<LittleEndian>(0)?;
        body.write_u32::<LittleEndian>(len_u32(self.tags.len())?)?;
        for tag in &self.tags {
            write_string(&mut body, tag)?;
        }
        body.write_u32::<LittleEndian>(len_u32(self.entries.len())?)?;
        for entry in &self.entries {
            if (entry.flags & RdbEntry::META != 0) != entry.meta.is_some() {
                return Err(RdbError::MetaFlagMismatch(entry.path.clone()).into());
            }
            write_string(&mut body, &entry.path)?;
            body.write_u16::<LittleEndian>(entry.flags)?;
            body.write_u32::<LittleEndian>(entry.tags)?;
            body.write_u64::<LittleEndian>(entry.guid)?;
            if let Some(meta) = entry.meta {
                body.write_u32::<LittleEndian>(meta.modified.0)?;
                body.write_u32::<LittleEndian>(meta.size)?;
            }
        }

        let form_size = len_u32(body.len() + 4)?;
        body[4..12].copy_from_slice(&(u64::from(form_size) + 8).to_le_bytes());
        FormHeader {
            size: form_size,
            form_type: *RDB_FORM_TYPE,
        }
        .write(&mut NoSeek::new(&mut *out))?;
        out.write_all(&body)?;
        Ok(())
    }

    /// The entry with a GUID, `None` for `0`. Searches all entries, use [`Rdb::guid_map`] for
    /// many lookups.
    #[must_use]
    pub fn find_guid(&self, guid: u64) -> Option<&RdbEntry> {
        if guid == 0 {
            return None;
        }
        self.entries.iter().find(|e| e.guid == guid)
    }

    /// The entry at a path, ASCII case insensitive, `\` is accepted as separator.
    #[must_use]
    pub fn find_path(&self, path: &str) -> Option<&RdbEntry> {
        let path = path.replace('\\', "/");
        let path = path.trim_matches('/');
        self.entries
            .iter()
            .find(|e| e.path.eq_ignore_ascii_case(path))
    }

    /// The entry a reference points to: by GUID if it has one, otherwise by path.
    #[must_use]
    pub fn resolve(&self, reference: &ResourceRef) -> Option<&RdbEntry> {
        reference.guid.map_or_else(
            || self.find_path(&reference.path),
            |guid| self.find_guid(guid),
        )
    }

    /// All entries with a GUID, by GUID. If GUIDs repeat, the last entry wins.
    #[must_use]
    pub fn guid_map(&self) -> HashMap<u64, &RdbEntry> {
        self.entries
            .iter()
            .filter(|e| e.guid != 0)
            .map(|e| (e.guid, e))
            .collect()
    }

    /// The names of an entry's tags. Bits without a tag are skipped.
    pub fn entry_tags<'a>(&'a self, entry: &RdbEntry) -> impl Iterator<Item = &'a str> + use<'a> {
        let mask = entry.tags;
        self.tags
            .iter()
            .take(32)
            .enumerate()
            .filter(move |(i, _)| mask & (1 << i) != 0)
            .map(|(_, tag)| tag.as_str())
    }
}

// The data is in memory, so reads only fail at its end.
fn read_u16(input: &mut Cursor<&[u8]>) -> Result<u16, RdbError> {
    input
        .read_u16::<LittleEndian>()
        .map_err(|_| RdbError::Truncated)
}

fn read_u32(input: &mut Cursor<&[u8]>) -> Result<u32, RdbError> {
    input
        .read_u32::<LittleEndian>()
        .map_err(|_| RdbError::Truncated)
}

fn read_u64(input: &mut Cursor<&[u8]>) -> Result<u64, RdbError> {
    input
        .read_u64::<LittleEndian>()
        .map_err(|_| RdbError::Truncated)
}

fn read_string(input: &mut Cursor<&[u8]>) -> Result<String, RdbError> {
    let len = read_u32(input)? as usize;
    let start = input.position() as usize;
    let bytes = start
        .checked_add(len)
        .and_then(|end| input.get_ref().get(start..end))
        .ok_or(RdbError::Truncated)?;
    let Some((&0, text)) = bytes.split_last() else {
        return Err(RdbError::InvalidString);
    };
    let text = std::str::from_utf8(text)
        .map_err(|_| RdbError::InvalidString)?
        .to_owned();
    input.set_position((start + len) as u64);
    Ok(text)
}

fn write_string(out: &mut Vec<u8>, text: &str) -> Result<(), RdbError> {
    out.extend_from_slice(&len_u32(text.len() + 1)?.to_le_bytes());
    out.extend_from_slice(text.as_bytes());
    out.push(0);
    Ok(())
}

fn len_u32(len: usize) -> Result<u32, RdbError> {
    u32::try_from(len).map_err(|_| RdbError::TooLarge)
}
