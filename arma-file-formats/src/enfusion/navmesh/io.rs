//! Little endian readers and writers over byte slices shared by the navmesh files, and the
//! chunk walking of their `FORM`s.

use std::io::{Read, Seek};

use crate::{core::iff::FormHeader, errors::NavmeshError};

/// Reads little endian values from a slice, [`NavmeshError::Truncated`] past its end.
pub(super) struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bytes<'a> {
    pub(super) const fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(super) const fn position(&self) -> usize {
        self.pos
    }

    pub(super) const fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub(super) fn take(&mut self, len: usize) -> Result<&'a [u8], NavmeshError> {
        let bytes = self
            .pos
            .checked_add(len)
            .and_then(|end| self.data.get(self.pos..end))
            .ok_or(NavmeshError::Truncated)?;
        self.pos += len;
        Ok(bytes)
    }

    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N], NavmeshError> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    pub(super) fn u8(&mut self) -> Result<u8, NavmeshError> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, NavmeshError> {
        self.array().map(u16::from_le_bytes)
    }

    pub(super) fn u32(&mut self) -> Result<u32, NavmeshError> {
        self.array().map(u32::from_le_bytes)
    }

    pub(super) fn i32(&mut self) -> Result<i32, NavmeshError> {
        self.array().map(i32::from_le_bytes)
    }

    pub(super) fn u64(&mut self) -> Result<u64, NavmeshError> {
        self.array().map(u64::from_le_bytes)
    }

    pub(super) fn f32(&mut self) -> Result<f32, NavmeshError> {
        self.array().map(f32::from_le_bytes)
    }

    pub(super) fn vec3(&mut self) -> Result<[f32; 3], NavmeshError> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    /// A count that is followed by at least `min_item_size` bytes per item, so that untrusted
    /// counts can't allocate more than the data holds.
    pub(super) fn count(&mut self, min_item_size: usize) -> Result<usize, NavmeshError> {
        let count = self.i32()?;
        let count = usize::try_from(count).map_err(|_| NavmeshError::Truncated)?;
        if count.saturating_mul(min_item_size) > self.remaining() {
            return Err(NavmeshError::Truncated);
        }
        Ok(count)
    }
}

/// Little endian writes into a byte vector.
pub(super) trait PutLe {
    fn put_u8(&mut self, v: u8);
    fn put_u16(&mut self, v: u16);
    fn put_u32(&mut self, v: u32);
    fn put_i32(&mut self, v: i32);
    fn put_u64(&mut self, v: u64);
    fn put_f32(&mut self, v: f32);
    fn put_vec3(&mut self, v: [f32; 3]);
}

impl PutLe for Vec<u8> {
    fn put_u8(&mut self, v: u8) {
        self.push(v);
    }
    fn put_u16(&mut self, v: u16) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_u32(&mut self, v: u32) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_i32(&mut self, v: i32) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_u64(&mut self, v: u64) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_f32(&mut self, v: f32) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_vec3(&mut self, v: [f32; 3]) {
        for c in v {
            self.put_f32(c);
        }
    }
}

pub(super) fn len_i32(len: usize) -> Result<i32, NavmeshError> {
    i32::try_from(len).map_err(|_| NavmeshError::TooLarge)
}

pub(super) fn len_u32(len: usize) -> Result<u32, NavmeshError> {
    u32::try_from(len).map_err(|_| NavmeshError::TooLarge)
}

/// Reads a `FORM` of `form_type` and returns its body after the form type.
pub(super) fn read_form<I: Read + Seek>(
    input: &mut I,
    form_type: [u8; 4],
) -> Result<Vec<u8>, crate::errors::AffError> {
    let form = FormHeader::read_form(input)?;
    if form.form_type != form_type {
        return Err(NavmeshError::UnknownFormType(form.form_type_lossy()).into());
    }
    let body_size = u64::from(form.size.saturating_sub(4));
    let mut body = Vec::new();
    input.take(body_size).read_to_end(&mut body)?;
    if body.len() as u64 != body_size {
        return Err(NavmeshError::Truncated.into());
    }
    Ok(body)
}

/// Splits a form body into its chunks, which have to be exactly `tags` in this order.
pub(super) fn chunks<'a, const N: usize>(
    body: &'a [u8],
    tags: [&'static str; N],
) -> Result<[&'a [u8]; N], NavmeshError> {
    let mut out = [&[][..]; N];
    let mut input = Bytes::new(body);
    for (slot, tag) in out.iter_mut().zip(tags) {
        if input.remaining() == 0 {
            return Err(NavmeshError::ChunkMissing(tag));
        }
        let found = input.array::<4>()?;
        if found != *tag.as_bytes() {
            return Err(NavmeshError::UnexpectedChunk {
                expected: tag,
                found: String::from_utf8_lossy(&found).into_owned(),
            });
        }
        let size = u32::from_be_bytes(input.array()?) as usize;
        *slot = input.take(size)?;
    }
    if input.remaining() != 0 {
        return Err(NavmeshError::TrailingData);
    }
    Ok(out)
}

/// Appends a chunk with a big endian size.
pub(super) fn put_chunk(out: &mut Vec<u8>, tag: [u8; 4], data: &[u8]) -> Result<(), NavmeshError> {
    out.extend_from_slice(&tag);
    out.extend_from_slice(&len_u32(data.len())?.to_be_bytes());
    out.extend_from_slice(data);
    Ok(())
}
