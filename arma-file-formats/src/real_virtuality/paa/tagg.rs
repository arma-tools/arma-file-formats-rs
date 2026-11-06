use std::io::{self, Read, Seek, Write};

use crate::core::{read::ReadExtTrait, write::WriteExtTrait};

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Tagg {
    pub signature: String,
    pub data: Vec<u8>,
}

impl Tagg {
    const TAGG_SIG_SIZE: usize = 8;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read<T>(&mut self, stream: &mut T) -> io::Result<()>
    where
        T: Read + Seek,
    {
        self.signature = stream.read_string(Self::TAGG_SIG_SIZE)?;
        let size = stream.read_u32()?;
        // Grows with the data, a garbage size doesn't allocate up to 4 GB.
        self.data.clear();
        if stream.take(size.into()).read_to_end(&mut self.data)? != size as usize {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(())
    }

    pub fn write<T>(&self, stream: &mut T) -> io::Result<()>
    where
        T: Write + Seek,
    {
        stream.write_string(&self.signature)?;
        stream.write_u32(self.data.len() as u32)?;
        stream.write_bytes(&self.data)?;

        Ok(())
    }
}
