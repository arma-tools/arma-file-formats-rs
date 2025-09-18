use std::io::{Read, Seek};

use crate::errors::AffError;

use super::{Entry, Pbo};

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct PboReader<R>
where
    R: Read + Seek,
{
    reader: R,
    pub pbo: Pbo,
}

impl<R> PboReader<R>
where
    R: Read + Seek,
{
    pub fn from_stream(reader: R) -> Result<Self, AffError> {
        let mut pbo_reader = Self {
            reader,
            pbo: Pbo::new(),
        };
        pbo_reader.pbo.read(&mut pbo_reader.reader, true)?;
        Ok(pbo_reader)
    }

    pub fn get_entry(&mut self, entry_path: &str) -> Result<Option<Entry>, AffError> {
        self.pbo.get_entry(entry_path, &mut self.reader)
    }

    pub fn get_prefix(&self) -> String {
        self.pbo.get_prefix()
    }

    pub fn extract_single_file(
        &mut self,
        entry_path: &str,
        out_path: &str,
        full_path: bool,
    ) -> Result<(), AffError> {
        self.pbo
            .extract_single_file(entry_path, out_path, full_path, &mut self.reader)
    }
}
