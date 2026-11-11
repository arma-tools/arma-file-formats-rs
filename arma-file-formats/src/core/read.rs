use std::io::{self, ErrorKind, Read, Seek, SeekFrom};

use binrw::{BinRead, BinResult, NullString};
use byteorder::{BigEndian, LittleEndian};

pub(crate) trait ReadExtTrait: Read + Seek {
    fn read_compressed_int(&mut self) -> io::Result<u32>;

    #[allow(unused)]
    fn read_bool(&mut self) -> io::Result<bool>;

    fn read_u8(&mut self) -> io::Result<u8>;
    fn read_u16(&mut self) -> io::Result<u16>;
    fn read_i32(&mut self) -> io::Result<i32>;
    fn read_u32(&mut self) -> io::Result<u32>;
    fn read_u32_be(&mut self) -> io::Result<u32>;

    fn read_f32(&mut self) -> io::Result<f32>;

    fn read_u24(&mut self) -> io::Result<u32>;

    fn read_bytes(&mut self, size: usize) -> io::Result<Vec<u8>>;

    fn read_string(&mut self, size: usize) -> io::Result<String>;

    fn read_string_lossy(&mut self, size: usize) -> io::Result<String>;
    fn read_string_zt(&mut self) -> BinResult<String>;
    fn peek_u8(&mut self) -> io::Result<u8>;

    fn peek_u16(&mut self) -> io::Result<u16>;

    #[allow(unused)]
    fn peek_string(&mut self, size: usize) -> io::Result<String>;

    fn peek_string_lossy(&mut self, size: usize) -> io::Result<String>;
}

impl<T> ReadExtTrait for T
where
    T: Read + Seek,
{
    /// 7 bits per byte, low bits first, the high bit marks a following byte.
    fn read_compressed_int(&mut self) -> io::Result<u32> {
        let mut ret = 0_u32;
        for shift in (0..35).step_by(7) {
            let val = ReadExtTrait::read_u8(self)?;
            ret |= u32::from(val & 0x7f).checked_shl(shift).unwrap_or(0);
            if val & 0x80 == 0 {
                return Ok(ret);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "compressed integer longer than 5 bytes",
        ))
    }

    fn read_bool(&mut self) -> io::Result<bool> {
        Ok(ReadExtTrait::read_u8(self)? != 0)
    }

    fn read_u8(&mut self) -> io::Result<u8> {
        byteorder::ReadBytesExt::read_u8(self)
    }

    fn read_u16(&mut self) -> io::Result<u16> {
        byteorder::ReadBytesExt::read_u16::<LittleEndian>(self)
    }

    fn read_i32(&mut self) -> io::Result<i32> {
        byteorder::ReadBytesExt::read_i32::<LittleEndian>(self)
    }

    fn read_u32(&mut self) -> io::Result<u32> {
        byteorder::ReadBytesExt::read_u32::<LittleEndian>(self)
    }

    fn read_u32_be(&mut self) -> io::Result<u32> {
        byteorder::ReadBytesExt::read_u32::<BigEndian>(self)
    }

    fn read_f32(&mut self) -> io::Result<f32> {
        byteorder::ReadBytesExt::read_f32::<LittleEndian>(self)
    }

    fn read_u24(&mut self) -> io::Result<u32> {
        byteorder::ReadBytesExt::read_u24::<LittleEndian>(self)
    }

    fn read_bytes(&mut self, size: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; size];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn read_string(&mut self, size: usize) -> io::Result<String> {
        let buf = self.read_bytes(size)?;
        let str = String::from_utf8(buf);

        str.map_or_else(
            |_| {
                Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "Invalid UTF8 String",
                ))
            },
            Ok,
        )
    }

    fn read_string_lossy(&mut self, size: usize) -> io::Result<String> {
        let buf = self.read_bytes(size)?;
        let str = String::from_utf8_lossy(&buf);
        Ok(str.to_string())
    }

    fn read_string_zt(&mut self) -> BinResult<String> {
        Ok(NullString::read(self)?.to_string())
    }

    fn peek_u8(&mut self) -> io::Result<u8> {
        let pos = self.stream_position()?;
        let ret = ReadExtTrait::read_u8(self)?;
        self.seek(SeekFrom::Start(pos))?;
        Ok(ret)
    }

    fn peek_u16(&mut self) -> io::Result<u16> {
        let pos = self.stream_position()?;
        let ret = ReadExtTrait::read_u16(self)?;
        self.seek(SeekFrom::Start(pos))?;
        Ok(ret)
    }

    fn peek_string(&mut self, size: usize) -> io::Result<String> {
        let pos = self.stream_position()?;
        let ret = self.read_string(size)?;
        self.seek(SeekFrom::Start(pos))?;
        Ok(ret)
    }

    fn peek_string_lossy(&mut self, size: usize) -> io::Result<String> {
        let pos = self.stream_position()?;
        let ret = self.read_string_lossy(size)?;
        self.seek(SeekFrom::Start(pos))?;
        Ok(ret)
    }
}
