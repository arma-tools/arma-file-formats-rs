use std::io::{self, Seek, SeekFrom, Write};

use byteorder::LittleEndian;

pub(crate) trait WriteExtTrait: Write + Seek {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64>;

    fn write_u8(&mut self, n: u8) -> io::Result<()>;
    fn write_u16(&mut self, n: u16) -> io::Result<()>;

    fn write_u24(&mut self, n: u32) -> io::Result<()>;

    fn write_u32(&mut self, n: u32) -> io::Result<()>;

    fn write_bytes(&mut self, buf: &[u8]) -> io::Result<()>;

    fn write_string(&mut self, str: &str) -> io::Result<()>;

    fn write_string_zt(&mut self, str: &str) -> io::Result<()>;
}

impl<T> WriteExtTrait for T
where
    T: Write + Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.seek(pos)
    }

    fn write_u8(&mut self, n: u8) -> io::Result<()> {
        byteorder::WriteBytesExt::write_u8(self, n)
    }
    fn write_u16(&mut self, n: u16) -> io::Result<()> {
        byteorder::WriteBytesExt::write_u16::<LittleEndian>(self, n)
    }

    fn write_u24(&mut self, n: u32) -> io::Result<()> {
        byteorder::WriteBytesExt::write_u24::<LittleEndian>(self, n)
    }

    fn write_u32(&mut self, n: u32) -> io::Result<()> {
        byteorder::WriteBytesExt::write_u32::<LittleEndian>(self, n)
    }

    fn write_bytes(&mut self, buf: &[u8]) -> io::Result<()> {
        self.write_all(buf)
    }

    fn write_string(&mut self, str: &str) -> io::Result<()> {
        self.write_all(str.as_bytes())
    }

    fn write_string_zt(&mut self, str: &str) -> io::Result<()> {
        self.write_all(str.as_bytes())?;
        WriteExtTrait::write_u8(self, 0)
    }
}
