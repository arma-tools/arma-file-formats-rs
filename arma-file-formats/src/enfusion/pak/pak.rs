use std::{
    cell::RefCell,
    fs::File,
    io::{self, BufReader, Cursor, Read, Seek, SeekFrom, Take},
    path::Path,
};

use binrw::BinRead;

use crate::{
    core::read::ReadExtTrait,
    enfusion::pak::{
        PakCompression, PakEntry, PakEntryReader, PakTimestamp,
        file_tree::{
            COMPRESSION_STORED, COMPRESSION_ZLIB, FORM_TYPE, FileRecord, HEAD_SIZE, HEAD_VERSION,
            Node, crc32, encode_head,
        },
    },
    errors::{AffError, PakError},
};

/// Table of contents of an Enfusion archive (`.pak`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Pak {
    /// Every file, in the order of the pak directory tree (which is also the data order).
    pub entries: Vec<PakEntry>,
}

impl Pak {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, AffError> {
        Self::from_stream(&mut BufReader::new(File::open(path)?))
    }

    /// Reads the `HEAD` and `FILE` chunks. File data is not read, use [`PakEntry::read`] or
    /// [`PakReader`] for that.
    pub fn from_stream<R: Read + Seek>(reader: &mut R) -> Result<Self, AffError> {
        if reader.read_string_lossy(4)? != "FORM" {
            return Err(AffError::InvalidFileError);
        }
        let form_end = u64::from(reader.read_u32_be()?) + 8;

        let mut form_type = [0; 4];
        reader.read_exact(&mut form_type)?;
        if &form_type != FORM_TYPE {
            return Err(PakError::UnknownFormType(
                String::from_utf8_lossy(&form_type).into_owned(),
            )
            .into());
        }

        let mut head = None;
        let mut file = None;
        while reader.stream_position()? < form_end {
            let mut tag = [0; 4];
            reader.read_exact(&mut tag)?;
            let size = reader.read_u32_be()?;
            match &tag {
                b"HEAD" => head = Some(reader.read_bytes(size as usize)?),
                b"FILE" => file = Some(reader.read_bytes(size as usize)?),
                _ => {
                    reader.seek(SeekFrom::Current(i64::from(size)))?;
                }
            }
        }

        let head = head.ok_or(PakError::ChunkMissing("HEAD"))?;
        let file = file.ok_or(PakError::ChunkMissing("FILE"))?;

        let version = head
            .get(..4)
            .map(|v| u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
            .ok_or(PakError::InvalidHead)?;
        if version != HEAD_VERSION {
            return Err(PakError::UnsupportedVersion(version).into());
        }
        if head.len() != HEAD_SIZE {
            return Err(PakError::InvalidHead.into());
        }
        if head != encode_head(crc32(&file)) {
            return Err(PakError::ChecksumMismatch.into());
        }

        let root = Node::read(&mut Cursor::new(&file))?;
        let mut entries = Vec::new();
        match root {
            Node::Dir { children, .. } => {
                for child in children {
                    flatten(child, "", &mut entries)?;
                }
            }
            Node::File { .. } => return Err(AffError::InvalidFileError),
        }

        Ok(Self { entries })
    }

    /// Finds an entry by path. Matching is ASCII case insensitive, `\` is accepted as separator
    /// and a leading separator is ignored.
    #[must_use]
    pub fn entry(&self, path: &str) -> Option<&PakEntry> {
        let path = path.replace('\\', "/");
        let path = path.trim_start_matches('/');
        self.entries
            .iter()
            .find(|e| e.path.eq_ignore_ascii_case(path))
    }
}

fn flatten(node: Node, parent: &str, entries: &mut Vec<PakEntry>) -> Result<(), AffError> {
    let join = |name: Vec<u8>| -> Result<String, AffError> {
        let name = String::from_utf8(name)?;
        Ok(if parent.is_empty() {
            name
        } else {
            format!("{parent}/{name}")
        })
    };

    match node {
        Node::Dir { name, children } => {
            let path = join(name.bytes)?;
            for child in children {
                flatten(child, &path, entries)?;
            }
        }
        Node::File { name, record } => {
            let path = join(name.bytes)?;
            entries.push(entry_from_record(path, &record)?);
        }
    }
    Ok(())
}

fn entry_from_record(path: String, record: &FileRecord) -> Result<PakEntry, PakError> {
    let compression = match record.compression {
        COMPRESSION_STORED => PakCompression::Stored,
        COMPRESSION_ZLIB => PakCompression::Zlib {
            level: record.compression_level,
        },
        other => return Err(PakError::UnknownCompression(other)),
    };

    Ok(PakEntry {
        path,
        offset: record.offset,
        compressed_size: record.compressed_size,
        size: record.size,
        compression,
        timestamp: PakTimestamp(record.timestamp),
    })
}

/// A [`Pak`] together with the stream it was read from.
///
/// File data is only read when an entry is opened or read. Any number of entries can be open at
/// once, they share the stream and each keeps its own position.
#[derive(Debug)]
pub struct PakReader<R>
where
    R: Read + Seek,
{
    shared: RefCell<Shared<R>>,
    pub pak: Pak,
}

#[derive(Debug)]
struct Shared<R> {
    reader: R,
    /// Position of `reader`, `None` when unknown.
    position: Option<u64>,
}

impl<R> PakReader<R>
where
    R: Read + Seek,
{
    pub fn from_stream(mut reader: R) -> Result<Self, AffError> {
        let pak = Pak::from_stream(&mut reader)?;
        Ok(Self {
            shared: RefCell::new(Shared {
                reader,
                position: None,
            }),
            pak,
        })
    }

    #[must_use]
    pub fn into_inner(self) -> R {
        self.shared.into_inner().reader
    }

    /// Opens the file at `path`, see [`Pak::entry`] and [`PakEntry::open`].
    pub fn open(&self, path: &str) -> Result<Option<PakEntryReader<PakStream<'_, R>>>, AffError> {
        self.pak
            .entry(path)
            .map(|entry| self.open_entry(entry))
            .transpose()
    }

    /// See [`PakEntry::open`].
    pub fn open_entry(
        &self,
        entry: &PakEntry,
    ) -> Result<PakEntryReader<PakStream<'_, R>>, AffError> {
        Ok(entry.open(self.stream())?)
    }

    /// See [`PakEntry::open_raw`].
    pub fn open_entry_raw(&self, entry: &PakEntry) -> Result<Take<PakStream<'_, R>>, AffError> {
        Ok(entry.open_raw(self.stream())?)
    }

    /// Reads the contents of the file at `path`, see [`Pak::entry`].
    pub fn read(&self, path: &str) -> Result<Option<Vec<u8>>, AffError> {
        self.pak
            .entry(path)
            .map(|entry| self.read_entry(entry))
            .transpose()
    }

    pub fn read_entry(&self, entry: &PakEntry) -> Result<Vec<u8>, AffError> {
        entry.read(&mut self.stream())
    }

    pub fn read_entry_raw(&self, entry: &PakEntry) -> Result<Vec<u8>, AffError> {
        entry.read_raw(&mut self.stream())
    }

    const fn stream(&self) -> PakStream<'_, R> {
        PakStream {
            shared: &self.shared,
            position: 0,
        }
    }
}

/// Cursor over the stream of a [`PakReader`], with its own position.
#[derive(Debug)]
pub struct PakStream<'a, R> {
    shared: &'a RefCell<Shared<R>>,
    position: u64,
}

impl<R: Read + Seek> Read for PakStream<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut shared = self.shared.borrow_mut();
        let Shared { reader, position } = &mut *shared;

        // Seeking discards buffers, so only seek when another cursor moved the stream.
        if *position != Some(self.position) {
            *position = None;
            reader.seek(SeekFrom::Start(self.position))?;
        }
        *position = None;
        let n = reader.read(buf)?;
        self.position += n as u64;
        *position = Some(self.position);
        Ok(n)
    }
}

impl<R: Read + Seek> Seek for PakStream<'_, R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.position = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(offset) => {
                self.position.checked_add_signed(offset).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid seek position")
                })?
            }
            SeekFrom::End(_) => {
                let mut shared = self.shared.borrow_mut();
                shared.position = None;
                let position = shared.reader.seek(pos)?;
                shared.position = Some(position);
                position
            }
        };
        Ok(self.position)
    }
}

impl PakReader<BufReader<File>> {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, AffError> {
        Self::from_stream(BufReader::new(File::open(path)?))
    }
}
