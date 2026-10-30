use std::io::{self, Read, Seek, SeekFrom, Take};

use flate2::read::ZlibDecoder;

use crate::{
    enfusion::pak::PakTimestamp,
    errors::{AffError, PakError},
};

/// Upper bound for preallocated read buffers, since sizes in a pak are not trusted.
const MAX_PREALLOC: u32 = 1 << 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PakCompression {
    Stored,
    /// zlib stream compressed at the given level (Workbench uses 6).
    Zlib {
        level: u8,
    },
}

/// File in a pak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PakEntry {
    /// Path inside the pak, components separated by `/`, without a leading slash.
    pub path: String,
    /// Absolute position of the data in the pak file.
    pub offset: u32,
    /// Size of the data in the pak, equal to [`Self::size`] for stored entries.
    pub compressed_size: u32,
    pub size: u32,
    pub compression: PakCompression,
    pub timestamp: PakTimestamp,
}

impl PakEntry {
    /// Opens the data as it is stored in the pak, without decompressing it.
    pub fn open_raw<R: Read + Seek>(&self, mut reader: R) -> io::Result<Take<R>> {
        reader.seek(SeekFrom::Start(u64::from(self.offset)))?;
        Ok(reader.take(u64::from(self.compressed_size)))
    }

    /// Opens the file contents, decompressing them while they are read. Reading fails once the
    /// contents turn out to have a different size than [`Self::size`].
    pub fn open<R: Read + Seek>(&self, reader: R) -> io::Result<PakEntryReader<R>> {
        let raw = self.open_raw(reader)?;
        let decoder = match self.compression {
            PakCompression::Stored => Decoder::Stored(raw),
            PakCompression::Zlib { .. } => Decoder::Zlib(ZlibDecoder::new(raw)),
        };
        Ok(PakEntryReader {
            decoder,
            path: self.path.clone(),
            size: u64::from(self.size),
            read: 0,
        })
    }

    /// Reads the data as it is stored in the pak, without decompressing it.
    pub fn read_raw<R: Read + Seek>(&self, reader: &mut R) -> Result<Vec<u8>, AffError> {
        let mut data = Vec::with_capacity(self.compressed_size.min(MAX_PREALLOC) as usize);
        self.open_raw(reader)?.read_to_end(&mut data)?;
        if data.len() != self.compressed_size as usize {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
        }
        Ok(data)
    }

    /// Reads and decompresses the file contents.
    pub fn read<R: Read + Seek>(&self, reader: &mut R) -> Result<Vec<u8>, AffError> {
        let mut data = Vec::with_capacity(self.size.min(MAX_PREALLOC) as usize);
        self.open(reader)?
            .read_to_end(&mut data)
            .map_err(from_io_error)?;
        Ok(data)
    }
}

/// Contents of a pak entry, decompressed while reading, see [`PakEntry::open`].
#[derive(Debug)]
pub struct PakEntryReader<R> {
    decoder: Decoder<R>,
    path: String,
    size: u64,
    read: u64,
}

#[derive(Debug)]
enum Decoder<R> {
    Stored(Take<R>),
    Zlib(ZlibDecoder<Take<R>>),
}

impl<R: Read> Read for PakEntryReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // One byte more than what is left, to notice contents that are too long.
        let left = self.size.saturating_sub(self.read) + 1;
        let len = usize::try_from(left).map_or(buf.len(), |left| left.min(buf.len()));
        let buf = &mut buf[..len];
        let n = match &mut self.decoder {
            Decoder::Stored(reader) => reader.read(buf)?,
            Decoder::Zlib(reader) => reader.read(buf)?,
        };

        self.read += n as u64;
        if self.read > self.size || (n == 0 && self.read != self.size) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                PakError::SizeMismatch {
                    path: self.path.clone(),
                    expected: self.size,
                    actual: self.read,
                },
            ));
        }
        Ok(n)
    }
}

/// Unwraps a [`PakError`] that was passed through an [`io::Error`].
pub(super) fn from_io_error(error: io::Error) -> AffError {
    if let Some(pak_error) = error.get_ref().and_then(|e| e.downcast_ref::<PakError>()) {
        return pak_error.clone().into();
    }
    error.into()
}
