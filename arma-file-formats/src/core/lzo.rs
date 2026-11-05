//! LZO1X decompression with bounds checks, for data that may be garbage (obfuscated PBOs carry
//! decoy files with valid headers and random contents).

use std::io::{self, Read};

use crate::errors::LzoError;

/// Decompresses one LZO1X stream from `reader` into exactly `expected_size` bytes.
///
/// The output only grows as input is read, so a garbage `expected_size` doesn't allocate more than
/// the input can produce. Fails instead of panicking on matches that reach before the start of the
/// output, on output beyond `expected_size` and on streams without an end marker.
pub fn decompress_lzo<R: Read>(reader: &mut R, expected_size: usize) -> Result<Vec<u8>, LzoError> {
    let mut out = Vec::with_capacity(expected_size.min(1 << 20));
    let mut state = 0;

    // First byte: 18..=255 is a literal run, anything else a regular instruction.
    let first = read_u8(reader)?;
    let mut next = if first >= 22 {
        copy_literals(reader, &mut out, usize::from(first - 17), expected_size)?;
        state = 4;
        None
    } else if first >= 18 {
        state = usize::from(first - 17);
        copy_literals(reader, &mut out, state, expected_size)?;
        None
    } else {
        Some(first)
    };

    loop {
        let inst = match next.take() {
            Some(inst) => inst,
            None => read_u8(reader)?,
        };
        let (distance, length, next_state);
        if inst & 0xc0 != 0 {
            // M2: 3-8 bytes within 2 kB.
            let h = usize::from(read_u8(reader)?);
            distance = (h << 3) + usize::from((inst >> 2) & 0x7) + 1;
            length = usize::from(inst >> 5) + 1;
            next_state = usize::from(inst & 0x3);
        } else if inst & 0x20 != 0 {
            // M3: within 16 kB.
            length = match usize::from(inst & 0x1f) {
                0 => 2 + 31 + zero_byte_length(reader)?,
                l => 2 + l,
            };
            let v = usize::from(read_u16(reader)?);
            distance = (v >> 2) + 1;
            next_state = v & 0x3;
        } else if inst & 0x10 != 0 {
            // M4: within 16..48 kB, distance 0 ends the stream.
            length = match usize::from(inst & 0x7) {
                0 => 2 + 7 + zero_byte_length(reader)?,
                l => 2 + l,
            };
            let v = usize::from(read_u16(reader)?);
            let d = (usize::from(inst & 0x8) << 11) + (v >> 2);
            if d == 0 {
                if length != 3 {
                    return Err(LzoError::InvalidEndMarker);
                }
                break;
            }
            distance = d + 16384;
            next_state = v & 0x3;
        } else if state == 0 {
            // Long literal run.
            let length = match usize::from(inst) {
                0 => 3 + 15 + zero_byte_length(reader)?,
                l => 3 + l,
            };
            copy_literals(reader, &mut out, length, expected_size)?;
            state = 4;
            continue;
        } else if state != 4 {
            // M1 after 1-3 literals: 2 bytes within 1 kB.
            let h = usize::from(read_u8(reader)?);
            distance = (h << 2) + usize::from(inst >> 2) + 1;
            length = 2;
            next_state = usize::from(inst & 0x3);
        } else {
            // M1 after 4 or more literals: 3 bytes within 2..3 kB.
            let h = usize::from(read_u8(reader)?);
            distance = (h << 2) + usize::from(inst >> 2) + 2049;
            length = 3;
            next_state = usize::from(inst & 0x3);
        }

        let start = out
            .len()
            .checked_sub(distance)
            .ok_or(LzoError::LookBehindOverrun)?;
        if out.len() + length > expected_size {
            return Err(LzoError::OutputOverrun);
        }
        // Matches can overlap their own output.
        for i in start..start + length {
            out.push(out[i]);
        }

        state = next_state;
        copy_literals(reader, &mut out, state, expected_size)?;
    }

    Ok(out)
}

fn copy_literals<R: Read>(
    reader: &mut R,
    out: &mut Vec<u8>,
    length: usize,
    expected_size: usize,
) -> Result<(), LzoError> {
    if out.len() + length > expected_size {
        return Err(LzoError::OutputOverrun);
    }
    let read = reader.take(length as u64).read_to_end(out)?;
    if read != length {
        return Err(LzoError::Io(io::ErrorKind::UnexpectedEof.into()));
    }
    Ok(())
}

/// Length extension of a match or literal run: 255 per zero byte, plus the first non-zero byte.
fn zero_byte_length<R: Read>(reader: &mut R) -> Result<usize, LzoError> {
    let mut length = 0usize;
    loop {
        match read_u8(reader)? {
            0 => length = length.checked_add(255).ok_or(LzoError::OutputOverrun)?,
            b => return Ok(length + usize::from(b)),
        }
    }
}

fn read_u8<R: Read>(reader: &mut R) -> io::Result<u8> {
    let mut b = [0];
    reader.read_exact(&mut b)?;
    Ok(b[0])
}

fn read_u16<R: Read>(reader: &mut R) -> io::Result<u16> {
    let mut b = [0; 2];
    reader.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn decompress(data: &[u8], size: usize) -> Result<Vec<u8>, LzoError> {
        decompress_lzo(&mut io::Cursor::new(data), size)
    }

    #[test]
    fn round_trip() {
        let data: Vec<u8> = (0..20_000u32)
            .flat_map(|i| [(i % 251) as u8, (i / 7) as u8, b'a', b'a', b'a'])
            .collect();
        let compressed = lzokay_native::compress(&data).unwrap();
        assert_eq!(decompress(&compressed, data.len()).unwrap(), data);
        assert!(matches!(
            decompress(&compressed, data.len() - 1),
            Err(LzoError::OutputOverrun)
        ));
    }

    #[test]
    fn garbage() {
        // 4 literals, then a match 2041 bytes back.
        assert!(matches!(
            decompress(&[0x15, 1, 2, 3, 4, 0x40, 0xff], 100),
            Err(LzoError::LookBehindOverrun)
        ));
        // Literals past the end of the input.
        assert!(matches!(
            decompress(&[0xff, 1, 2], 1000),
            Err(LzoError::Io(_))
        ));
        // No end marker.
        assert!(matches!(
            decompress(&[0x16, 1, 2, 3, 4, 5], 100),
            Err(LzoError::Io(_))
        ));
        // A huge expected size doesn't allocate it.
        assert!(decompress(&[0x16, 1, 2, 3, 4, 5], usize::MAX / 2).is_err());
    }
}
