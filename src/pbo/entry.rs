use std::io::{BufRead, Seek, SeekFrom, Write};

use crate::core::read::ReadExtTrait;
use crate::core::write::WriteExtTrait;
use crate::errors::RvffError;
#[derive(Debug, Clone)]
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
    #[must_use] pub fn new() -> Self {
        Self {
            filename: String::new(),
            mime_type: String::new(),
            original_size: 0,
            offset: 0,
            timestamp: 0,
            data_size: 0,
            data: Vec::new(),
            data_offset: 0,
        }
    }

    pub fn read<R>(&mut self, reader: &mut R) -> Result<(), RvffError>
    where
        R: BufRead + Seek,
    {
        self.filename = reader.read_string_zt()?.to_lowercase();
        self.mime_type = reader.read_string(4)?;
        self.original_size = reader.read_u32()?;
        self.offset = reader.read_u32()?;
        self.timestamp = reader.read_u32()?;
        self.data_size = reader.read_u32()?;

        Ok(())
    }
    pub fn read_data<R>(&mut self, reader: &mut R) -> Result<(), RvffError>
    where
        R: BufRead + Seek,
    {
        reader.seek(SeekFrom::Start(self.data_offset))?;
        self.data = reader.read_bytes(self.data_size as usize)?;
        Ok(())
    }

    pub fn write<R>(&mut self, writer: &mut R) -> Result<(), RvffError>
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

impl Default for Entry {
    fn default() -> Self {
        Self::new()
    }
}
