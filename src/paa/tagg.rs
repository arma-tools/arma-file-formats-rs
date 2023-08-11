use std::io::{self, BufRead, Seek, Write};

use crate::core::{read::ReadExtTrait, write::WriteExtTrait};

#[derive(Clone, Debug)]
pub struct Tagg {
    pub signature: String,
    pub data: Vec<u8>,
}

impl Tagg {
    const TAGG_SIG_SIZE: usize = 8;

    #[must_use] pub fn new() -> Self {
        Self {
            signature: String::new(),
            data: Vec::new(),
        }
    }

    pub fn read<T>(&mut self, stream: &mut T) -> io::Result<()>
    where
        T: BufRead + Seek,
    {
        self.signature = stream.read_string(Self::TAGG_SIG_SIZE)?;
        let size = stream.read_u32()? as usize;
        self.data = stream.read_bytes(size)?;

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

impl Default for Tagg {
    fn default() -> Self {
        Self::new()
    }
}
