use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use crate::{
    core::{decompress_lzss, read::ReadExtTrait, write::WriteExtTrait},
    errors::AffError,
};

const COMPRESSION_MAGIC: &str = "srpC";

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Entry {
    pub filename: String,
    pub mime_type: String,
    pub original_size: u32,
    pub(crate) offset: u32,
    pub timestamp: u32,
    pub(crate) data_size: u32,

    pub data: Vec<u8>,

    pub(crate) data_offset: u64,
}

impl Entry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read<R>(&mut self, reader: &mut R) -> Result<(), AffError>
    where
        R: Read + Seek,
    {
        self.filename = reader.read_string_zt()?.to_lowercase();
        self.mime_type = reader.read_string(4)?;
        self.original_size = reader.read_u32()?;
        self.offset = reader.read_u32()?;
        self.timestamp = reader.read_u32()?;
        self.data_size = reader.read_u32()?;

        Ok(())
    }

    pub fn read_data<R>(&mut self, reader: &mut R) -> Result<(), AffError>
    where
        R: Read + Seek,
    {
        reader.seek(SeekFrom::Start(self.data_offset))?;
        let data = reader.read_bytes(self.data_size as usize)?;

        self.data = if !self.mime_type.is_empty() && self.mime_type == COMPRESSION_MAGIC {
            match decompress_lzss(
                &mut Cursor::new(data.clone()),
                self.original_size as usize,
                false,
            ) {
                Ok((read_size, data)) => {
                    assert_eq!(read_size, self.data_size.into());
                    data
                }
                // High chance this is just obfuscation garbage
                #[cfg(debug_assertions)]
                Err(err) => {
                    println!(
                        "Lzss error '{}' at file '{}'. Possible obfuscation garbage",
                        err, self.filename
                    );
                    data
                }
                #[cfg(not(debug_assertions))]
                Err(_) => data,
            }
        } else {
            data
        };

        Ok(())
    }

    pub fn write<R>(&mut self, writer: &mut R) -> Result<(), AffError>
    where
        R: Write + Seek,
    {
        writer.write_string_zt(&self.filename)?;
        writer.write_string(&self.mime_type)?;
        writer.write_u32(self.original_size)?;
        writer.write_u32(self.offset)?;
        writer.write_u32(self.timestamp)?;
        writer.write_u32(self.data_size)?;

        Ok(())
    }
}
