use std::io::{BufRead, Seek};

use crate::errors::RvffError;

use super::{Entry, Pbo};

pub struct PboReader<R>
where
    R: BufRead + Seek,
{
    reader: R,
    pub pbo: Pbo,
}

impl<R> PboReader<R>
where
    R: BufRead + Seek,
{
    pub fn from_stream(reader: R) -> Result<Self, RvffError> {
        let mut pbo_reader = PboReader {
            reader,
            pbo: Pbo::new(),
        };

        pbo_reader.pbo.read(&mut pbo_reader.reader, true)?;
        Ok(pbo_reader)
    }

    pub fn get_entry(&mut self, entry_path: String) -> Result<Option<Entry>, RvffError> {
        self.pbo.get_entry(entry_path, &mut self.reader)
    }
}
