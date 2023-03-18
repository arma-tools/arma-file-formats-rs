use std::io::{Read, Seek};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::errors::RvffLzssError;

pub(crate) fn decompress_lzss<R>(
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
            num5 = val as i32 | 65280;
        }
        if (num5 & 1) != 0 {
            let val = reader.read_u8().unwrap();
            if use_signed_checksum {
                calculated_hash += val as i8 as i32;
            } else {
                calculated_hash += val as i32;
            }
            dst[num2 as usize] = val;
            num2 += 1;
            remaining_size -= 1;
            array[num4 as usize] = val;
            num4 += 1;
            num4 &= 4095;
        } else {
            let mut i = reader.read_u8().unwrap() as i32;
            let mut val = reader.read_u8().unwrap() as i32;
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
                    calculated_hash += num6 as i8 as i32;
                } else {
                    calculated_hash += num6 as i32;
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

    //let hash = vec![0_u8; 4];
    let hash = reader.read_i32::<LittleEndian>().unwrap();
    if hash != calculated_hash {
        return Err(RvffLzssError::ChecksumMissmatch);
    }
    let size = reader.stream_position().unwrap() - pos;
    Ok((size, dst))
}
