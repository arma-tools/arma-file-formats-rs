use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::Path,
    u8,
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::errors::{RvffError, RvffLzssError};

pub fn decompress_lzss<R>(
    reader: &mut R,
    expected_size: usize,
    use_signed_checksum: bool,
) -> Result<(u64, Vec<u8>), RvffLzssError>
where
    R: Read + Seek,
{
    let mut array = vec![0_u8; 4113];
    let mut dst = vec![0_u8; expected_size];

    if expected_size == 0 {
        return Ok((0, Vec::new()));
    }

    let pos = reader.stream_position().unwrap();

    let mut remaining_size = expected_size;
    let mut num2: i32 = 0;
    let mut calculated_hash: i32 = 0;

    array.iter_mut().take(4078).for_each(|el| *el = 0x20);

    let mut num4: i32 = 4078;
    let mut num5: i32 = 0;

    while remaining_size > 0 {
        num5 >>= 1;
        if (num5 & 256) == 0 {
            let val = reader.read_u8().unwrap();
            num5 = i32::from(val) | 65280;
        }
        if (num5 & 1) != 0 {
            let val = reader.read_u8().unwrap();
            if use_signed_checksum {
                calculated_hash += i32::from(val as i8);
            } else {
                calculated_hash = calculated_hash.overflowing_add(i32::from(val)).0;
            }
            dst[num2 as usize] = val;
            num2 += 1;
            remaining_size -= 1;
            array[num4 as usize] = val;
            num4 += 1;
            num4 &= 4095;
        } else {
            let mut i = i32::from(reader.read_u8().unwrap());
            let mut val = i32::from(reader.read_u8().unwrap());
            i |= (val & 240) << 4;
            val &= 15;
            val += 2;
            let mut j = num4 - i;
            let num8 = val + j;
            if (val + 1) as usize > remaining_size {
                return Err(RvffLzssError::Overflow);
            }
            while j <= num8 {
                let num6 = array[(j & 4095) as usize];
                if use_signed_checksum {
                    calculated_hash += i32::from(num6 as i8);
                } else {
                    calculated_hash = calculated_hash.overflowing_add(i32::from(num6)).0;
                }
                dst[num2 as usize] = num6;
                num2 += 1;
                remaining_size -= 1;
                array[num4 as usize] = num6;
                num4 += 1;
                num4 &= 4095;
                j += 1;
            }
        }
    }

    let hash = reader.read_i32::<LittleEndian>().unwrap();
    if hash != calculated_hash {
        return Err(RvffLzssError::ChecksumMissmatch);
    }
    let size = reader.stream_position().unwrap() - pos;
    Ok((size, dst))
}

pub fn decompress_lzss_unk_size<R>(reader: &mut R) -> Result<Vec<u8>, RvffError>
where
    R: Read + Seek,
{
    reader.seek(SeekFrom::End(0))?;
    let in_size = reader.stream_position()?;

    reader.rewind()?;

    let sliding_winding_size: i32 = 4096;
    let best_match: i32 = 18;
    let threshold: i32 = 2;

    let mut text_buffer = vec![0_u8; (sliding_winding_size + best_match - 1) as usize];
    let mut out = Vec::new();
    out.reserve(in_size as usize * 4);

    let mut check_sum = 0_i32;
    let mut flags = 0_i32;

    let mut text_buffer_index: i32 = sliding_winding_size - best_match;

    let mut size = 0;

    while reader.stream_position()? < (in_size - 4) {
        flags >>= 1;
        if (flags & 256) == 0 {
            flags = i32::from(reader.read_u8()?) | 0xff00;
        }
        if (flags & 1) != 0 {
            let data = reader.read_u8()?;
            check_sum = check_sum.overflowing_add(i32::from(data)).0;

            out.push(data);
            size += 1;

            text_buffer[text_buffer_index as usize] = data;
            text_buffer_index += 1;
            text_buffer_index &= sliding_winding_size - 1;
        } else {
            let mut pos: i32 = i32::from(reader.read_u8()?);
            let mut len: i32 = i32::from(reader.read_u8()?);

            pos |= (len & 0xf0) << 4;
            len &= 0x0f;
            len += threshold;

            let mut buffer_pos = text_buffer_index - pos;
            let buffer_len = len + buffer_pos;

            while buffer_pos <= buffer_len {
                let data = text_buffer[(buffer_pos & (sliding_winding_size - 1)) as usize];
                check_sum = check_sum.overflowing_add(i32::from(data)).0;
                out.push(data);
                size += 1;

                text_buffer[text_buffer_index as usize] = data;
                text_buffer_index += 1;
                text_buffer_index &= sliding_winding_size - 1;

                buffer_pos += 1;
            }
        }
    }

    let checksum_read = reader.read_i32::<LittleEndian>()?;
    if check_sum == checksum_read {
        out.truncate(size);
        Ok(out)
    } else {
        Err(RvffError::Unknown)
    }
}

pub fn check_for_magic_and_decompress_lzss<R>(
    reader: &mut R,
    magic: &[u8],
) -> Result<Option<Vec<u8>>, RvffError>
where
    R: Read + Seek,
{
    reader.rewind()?;
    dbg!(&reader.stream_position()?);
    reader.read_u8()?;
    dbg!(&reader.stream_position()?);
    let mut magic_buffer = vec![0_u8; magic.len()];
    reader.read_exact(&mut magic_buffer)?;
    dbg!(&reader.stream_position()?);
    reader.rewind()?;
    dbg!(&reader.stream_position()?);
    dbg!(&magic);
    dbg!(&magic_buffer);
    if magic == magic_buffer {
        let data = decompress_lzss_unk_size(reader)?;
        Ok(Some(data))
    } else {
        Ok(None)
    }
}

pub fn check_for_magic_and_decompress_lzss_file<P: AsRef<Path>>(
    path: P,
    magic: &[u8],
) -> Result<bool, RvffError> {
    let mut file = File::open(&path)?;
    let uncomp_data = check_for_magic_and_decompress_lzss(&mut file, magic)?;

    if let Some(data) = uncomp_data {
        fs::write(path, data)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
