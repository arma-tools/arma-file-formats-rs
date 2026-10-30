use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt,
    fs::File,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use binrw::BinWrite;
use flate2::{Compression, write::ZlibEncoder};

use crate::{
    enfusion::pak::{
        PakCompression, PakEntry, PakTimestamp,
        file_tree::{
            COMPRESSION_STORED, COMPRESSION_ZLIB, DATA_OFFSET, FORM_TYPE, FileRecord, HEAD_SIZE,
            Node, NodeName, crc32, encode_head,
        },
    },
    errors::{AffError, PakError},
};

/// zlib level used by Workbench.
pub const PAK_DEFAULT_ZLIB_LEVEL: u8 = 6;

/// Contents of a file added to a [`PakWriter`]. Paths and readers are only read when the pak is
/// written.
pub enum PakSource<'a> {
    Bytes(Cow<'a, [u8]>),
    /// File on disk, opened when the pak is written.
    Path(PathBuf),
    /// Read to the end when the pak is written.
    Reader(Box<dyn Read + 'a>),
}

impl<'a> PakSource<'a> {
    pub fn reader<R: Read + 'a>(reader: R) -> Self {
        Self::Reader(Box::new(reader))
    }

    fn copy_to<W: Write>(self, out: &mut W) -> io::Result<u64> {
        match self {
            Self::Bytes(bytes) => {
                out.write_all(&bytes)?;
                Ok(bytes.len() as u64)
            }
            Self::Path(path) => io::copy(&mut open(&path)?, out),
            Self::Reader(mut reader) => io::copy(&mut reader, out),
        }
    }

    fn into_bytes(self) -> io::Result<Cow<'a, [u8]>> {
        match self {
            Self::Bytes(bytes) => Ok(bytes),
            Self::Path(path) => {
                let mut bytes = Vec::new();
                open(&path)?.read_to_end(&mut bytes)?;
                Ok(bytes.into())
            }
            Self::Reader(mut reader) => {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes)?;
                Ok(bytes.into())
            }
        }
    }
}

fn open(path: &Path) -> io::Result<File> {
    File::open(path).map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))
}

impl fmt::Debug for PakSource<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bytes(bytes) => f.debug_tuple("Bytes").field(&bytes.len()).finish(),
            Self::Path(path) => f.debug_tuple("Path").field(path).finish(),
            Self::Reader(_) => f.write_str("Reader"),
        }
    }
}

impl<'a> From<&'a [u8]> for PakSource<'a> {
    fn from(bytes: &'a [u8]) -> Self {
        Self::Bytes(bytes.into())
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for PakSource<'a> {
    fn from(bytes: &'a [u8; N]) -> Self {
        Self::Bytes(bytes.as_slice().into())
    }
}

impl<'a> From<&'a Vec<u8>> for PakSource<'a> {
    fn from(bytes: &'a Vec<u8>) -> Self {
        Self::Bytes(bytes.into())
    }
}

impl From<Vec<u8>> for PakSource<'_> {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Bytes(bytes.into())
    }
}

impl<'a> From<Cow<'a, [u8]>> for PakSource<'a> {
    fn from(bytes: Cow<'a, [u8]>) -> Self {
        Self::Bytes(bytes)
    }
}

impl From<PathBuf> for PakSource<'_> {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

impl From<&Path> for PakSource<'_> {
    fn from(path: &Path) -> Self {
        Self::Path(path.to_owned())
    }
}

/// Builds an Enfusion archive (`.pak`).
///
/// Sources are read one file at a time in [`Self::write`] and streamed to the output, only files
/// added with [`Self::add_file`] are held in memory while they are compressed.
#[derive(Debug, Default)]
pub struct PakWriter<'a> {
    files: Vec<(String, PendingFile<'a>)>,
}

#[derive(Debug)]
struct PendingFile<'a> {
    mode: Mode,
    timestamp: PakTimestamp,
    source: PakSource<'a>,
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    /// Zlib if that is smaller, otherwise stored.
    Auto,
    Compression(PakCompression),
    /// Already stored data of another pak.
    Raw {
        compression: PakCompression,
        size: u32,
        compressed_size: u32,
    },
}

impl<'a> PakWriter<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file, zlib compressed at [`PAK_DEFAULT_ZLIB_LEVEL`] if that makes it smaller.
    ///
    /// Paths use `/` or `\` as separator. Paths have to be unique ignoring ASCII case.
    pub fn add_file(
        &mut self,
        path: &str,
        source: impl Into<PakSource<'a>>,
        timestamp: PakTimestamp,
    ) -> Result<(), AffError> {
        self.push(path, Mode::Auto, timestamp, source.into())
    }

    /// Adds a file with the given compression, see [`Self::add_file`].
    pub fn add_file_with_compression(
        &mut self,
        path: &str,
        source: impl Into<PakSource<'a>>,
        timestamp: PakTimestamp,
        compression: PakCompression,
    ) -> Result<(), AffError> {
        self.push(
            path,
            Mode::Compression(compression),
            timestamp,
            source.into(),
        )
    }

    /// Adds an entry of another pak without recompressing it. `raw` is the stored data, as
    /// returned by [`PakEntry::open_raw`].
    pub fn add_raw(
        &mut self,
        entry: &PakEntry,
        raw: impl Into<PakSource<'a>>,
    ) -> Result<(), AffError> {
        let mode = Mode::Raw {
            compression: entry.compression,
            size: entry.size,
            compressed_size: entry.compressed_size,
        };
        self.push(&entry.path, mode, entry.timestamp, raw.into())
    }

    fn push(
        &mut self,
        path: &str,
        mode: Mode,
        timestamp: PakTimestamp,
        source: PakSource<'a>,
    ) -> Result<(), AffError> {
        let path = path.replace('\\', "/");
        let path = path.trim_start_matches('/');
        if path
            .split('/')
            .any(|component| component.is_empty() || component.len() > usize::from(u8::MAX))
        {
            return Err(PakError::InvalidPath(path.to_owned()).into());
        }

        self.files.push((
            path.to_owned(),
            PendingFile {
                mode,
                timestamp,
                source,
            },
        ));
        Ok(())
    }

    /// Writes the pak at the current position of `out`. The sizes and the checksum are written
    /// after the data, which is why `out` has to be seekable.
    pub fn write<W: Write + Seek>(self, out: &mut W) -> Result<(), AffError> {
        let mut root = Dir::default();
        for (path, file) in self.files {
            root.insert(path, file)?;
        }

        let start = out.stream_position()?;
        out.write_all(b"FORM")?;
        out.write_all(&0u32.to_be_bytes())?;
        out.write_all(FORM_TYPE)?;
        out.write_all(b"HEAD")?;
        out.write_all(&(HEAD_SIZE as u32).to_be_bytes())?;
        out.write_all(&[0; HEAD_SIZE])?;
        out.write_all(b"DATA")?;
        out.write_all(&0u32.to_be_bytes())?;

        let mut offset = u64::from(DATA_OFFSET);
        let root = root.into_node(Vec::new(), out, &mut offset)?;

        let mut file_chunk = Cursor::new(Vec::new());
        root.write(&mut file_chunk)?;
        let file_chunk = file_chunk.into_inner();
        out.write_all(b"FILE")?;
        out.write_all(&(file_chunk.len() as u32).to_be_bytes())?;
        out.write_all(&file_chunk)?;

        let end = out.stream_position()?;
        let form_size = u32::try_from(end - start - 8).map_err(|_| PakError::TooLarge)?;
        let data_size = (offset - u64::from(DATA_OFFSET)) as u32;

        out.seek(SeekFrom::Start(start + 4))?;
        out.write_all(&form_size.to_be_bytes())?;
        out.seek(SeekFrom::Start(start + 20))?;
        out.write_all(&encode_head(crc32(&file_chunk)))?;
        out.seek(SeekFrom::Start(start + u64::from(DATA_OFFSET) - 4))?;
        out.write_all(&data_size.to_be_bytes())?;
        out.seek(SeekFrom::Start(end))?;

        Ok(())
    }
}

/// Directory while building the tree. Children are keyed by their ASCII uppercased name, which
/// is both the order Workbench writes them in and how name collisions are detected.
#[derive(Default)]
struct Dir<'a> {
    children: BTreeMap<String, (String, Child<'a>)>,
}

enum Child<'a> {
    Dir(Dir<'a>),
    /// Full path and file.
    File(String, PendingFile<'a>),
}

impl<'a> Dir<'a> {
    fn insert(&mut self, path: String, file: PendingFile<'a>) -> Result<(), PakError> {
        let mut dir = self;
        let mut components = path.split('/').peekable();
        while let Some(name) = components.next() {
            let key = name.to_ascii_uppercase();
            if components.peek().is_none() {
                if dir.children.contains_key(&key) {
                    break;
                }
                dir.children
                    .insert(key, (name.to_owned(), Child::File(path.clone(), file)));
                return Ok(());
            }

            let (_, child) = dir
                .children
                .entry(key)
                .or_insert_with(|| (name.to_owned(), Child::Dir(Self::default())));
            dir = match child {
                Child::Dir(child) => child,
                Child::File(..) => break,
            };
        }
        Err(PakError::DuplicatePath(path))
    }

    /// Writes the file data in tree order and returns the `FILE` node of the directory.
    fn into_node<W: Write>(
        self,
        name: Vec<u8>,
        out: &mut W,
        offset: &mut u64,
    ) -> Result<Node, AffError> {
        let children = self
            .children
            .into_values()
            .map(|(name, child)| match child {
                Child::Dir(dir) => dir.into_node(name.into_bytes(), out, offset),
                Child::File(path, file) => {
                    let record_offset = u32::try_from(*offset).map_err(|_| PakError::TooLarge)?;
                    let (record, written) = write_file(&path, file, out)?;
                    *offset += written;
                    Ok(Node::File {
                        name: NodeName {
                            bytes: name.into_bytes(),
                        },
                        record: FileRecord {
                            offset: record_offset,
                            ..record
                        },
                    })
                }
            })
            .collect::<Result<_, _>>()?;

        Ok(Node::Dir {
            name: NodeName { bytes: name },
            children,
        })
    }
}

/// Writes the data of a file, returns its record without the offset and the bytes written.
fn write_file<W: Write>(
    path: &str,
    file: PendingFile<'_>,
    out: &mut W,
) -> Result<(FileRecord, u64), AffError> {
    let too_large = |size: u64| u32::try_from(size).map_err(|_| PakError::TooLarge);

    let (compression, size, compressed_size) = match file.mode {
        Mode::Raw {
            compression,
            size,
            compressed_size,
        } => {
            let written = file.source.copy_to(out)?;
            if written != u64::from(compressed_size) {
                return Err(PakError::SizeMismatch {
                    path: path.to_owned(),
                    expected: u64::from(compressed_size),
                    actual: written,
                }
                .into());
            }
            (compression, size, compressed_size)
        }
        Mode::Compression(PakCompression::Stored) => {
            let size = too_large(file.source.copy_to(out)?)?;
            (PakCompression::Stored, size, size)
        }
        Mode::Compression(compression @ PakCompression::Zlib { level }) => {
            let mut encoder = ZlibEncoder::new(&mut *out, Compression::new(u32::from(level)));
            let size = too_large(file.source.copy_to(&mut encoder)?)?;
            encoder.try_finish()?;
            (compression, size, too_large(encoder.total_out())?)
        }
        Mode::Auto => {
            let data = file.source.into_bytes()?;
            let size = too_large(data.len() as u64)?;
            let compressed = zlib_compress(&data, PAK_DEFAULT_ZLIB_LEVEL)?;
            if compressed.len() < data.len() {
                out.write_all(&compressed)?;
                let compression = PakCompression::Zlib {
                    level: PAK_DEFAULT_ZLIB_LEVEL,
                };
                (compression, size, compressed.len() as u32)
            } else {
                out.write_all(&data)?;
                (PakCompression::Stored, size, size)
            }
        }
    };

    let (compression, compression_level) = match compression {
        PakCompression::Stored => (COMPRESSION_STORED, 0),
        PakCompression::Zlib { level } => (COMPRESSION_ZLIB, level),
    };
    let record = FileRecord {
        offset: 0,
        compressed_size,
        size,
        compression,
        compression_level,
        timestamp: file.timestamp.0,
    };
    Ok((record, u64::from(compressed_size)))
}

fn zlib_compress(data: &[u8], level: u8) -> Result<Vec<u8>, AffError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(u32::from(level)));
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}
