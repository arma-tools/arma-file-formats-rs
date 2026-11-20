//! Little endian reads that turn a short read into [`RtmError::Truncated`].

use std::io::Read;

use crate::errors::RtmError;

/// Most elements reserved up front: counts come from the file and can be garbage, a short
/// file then fails after reading what is there instead of allocating gigabytes first.
const MAX_RESERVE: usize = 1 << 12;

pub fn read_exact<R: Read, const N: usize>(input: &mut R) -> Result<[u8; N], RtmError> {
    let mut buf = [0; N];
    input
        .read_exact(&mut buf)
        .map_err(|_| RtmError::Truncated)?;
    Ok(buf)
}

pub fn read_u8<R: Read>(input: &mut R) -> Result<u8, RtmError> {
    Ok(read_exact::<R, 1>(input)?[0])
}

pub fn read_u32<R: Read>(input: &mut R) -> Result<u32, RtmError> {
    Ok(u32::from_le_bytes(read_exact(input)?))
}

pub fn read_f32<R: Read>(input: &mut R) -> Result<f32, RtmError> {
    Ok(f32::from_le_bytes(read_exact(input)?))
}

pub fn read_vec3<R: Read>(input: &mut R) -> Result<[f32; 3], RtmError> {
    Ok([read_f32(input)?, read_f32(input)?, read_f32(input)?])
}

/// `count` elements read by `read`.
pub fn read_array<R: Read, T>(
    input: &mut R,
    count: usize,
    mut read: impl FnMut(&mut R) -> Result<T, RtmError>,
) -> Result<Vec<T>, RtmError> {
    let mut out = Vec::with_capacity(count.min(MAX_RESERVE));
    for _ in 0..count {
        out.push(read(input)?);
    }
    Ok(out)
}

/// `size` bytes, growing the buffer with the data actually there.
pub fn read_bytes<R: Read>(input: &mut R, size: usize) -> Result<Vec<u8>, RtmError> {
    let mut data = Vec::with_capacity(size.min(1 << 20));
    input
        .take(size as u64)
        .read_to_end(&mut data)
        .map_err(|_| RtmError::Truncated)?;
    if data.len() == size {
        Ok(data)
    } else {
        Err(RtmError::Truncated)
    }
}

pub fn write_f32s(out: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}
