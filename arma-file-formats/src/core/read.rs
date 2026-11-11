use std::io::{self, ErrorKind, Read, Seek, SeekFrom};

use binrw::{BinRead, BinResult, NullString};
use byteorder::LittleEndian;

pub(crate) trait ReadExtTrait: Read + Seek {
    fn read_compressed_int(&mut self) -> io::Result<u32>;

    fn read_u8(&mut self) -> io::Result<u8>;
    fn read_u16(&mut self) -> io::Result<u16>;
    fn read_i32(&mut self) -> io::Result<i32>;
    fn read_u32(&mut self) -> io::Result<u32>;

    fn read_f32(&mut self) -> io::Result<f32>;

    fn read_u24(&mut self) -> io::Result<u32>;

    fn read_bytes(&mut self, size: usize) -> io::Result<Vec<u8>>;

    fn read_string(&mut self, size: usize) -> io::Result<String>;

    fn read_string_lossy(&mut self, size: usize) -> io::Result<String>;
    fn read_string_zt(&mut self) -> BinResult<String>;
    fn peek_u8(&mut self) -> io::Result<u8>;

    fn peek_u16(&mut self) -> io::Result<u16>;

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

    fn peek_string_lossy(&mut self, size: usize) -> io::Result<String> {
        let pos = self.stream_position()?;
        let ret = self.read_string_lossy(size)?;
        self.seek(SeekFrom::Start(pos))?;
        Ok(ret)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::io::Cursor;

    use super::ReadExtTrait;

    #[test]
    fn compressed_ints() {
        let read = |bytes: &[u8]| Cursor::new(bytes.to_vec()).read_compressed_int();
        assert_eq!(read(&[0x7F]).unwrap(), 127);
        assert_eq!(read(&[0x80, 0x01]).unwrap(), 128);
        assert_eq!(read(&[0xA0, 0x03]).unwrap(), 416);
        assert_eq!(read(&[0xFF, 0xFF, 0x03]).unwrap(), 0xFFFF);
        assert!(read(&[0x80; 6]).is_err());
        assert!(read(&[0x80]).is_err());
    }

    #[test]
    fn strings_and_peeks() {
        let mut c = Cursor::new(b"abc\xFF\0".to_vec());
        assert_eq!(c.peek_u8().unwrap(), b'a');
        assert_eq!(c.peek_u16().unwrap(), u16::from_le_bytes(*b"ab"));
        assert_eq!(c.peek_string_lossy(4).unwrap(), "abc\u{FFFD}");
        assert_eq!(c.read_string(3).unwrap(), "abc");
        assert!(c.read_string(1).is_err());
        assert_eq!(c.position(), 4);
    }
}
