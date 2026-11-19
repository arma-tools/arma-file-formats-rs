use std::io::{Read, Seek, Write};

use binrw::{BinWrite, io::NoSeek};

use crate::{
    core::iff::{ChunkHeader, FormHeader},
    errors::{AffError, StarsError},
};

const STARS_FORM_TYPE: &[u8; 4] = b"STAR";
const ITEM_TAG: &[u8; 4] = b"ITEM";
/// Bytes of an `ITEM` before the name: six `f32`, colour, Hipparcos number and name length.
const ITEM_FIXED_SIZE: usize = 36;

/// Enfusion star catalog (`.stars`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StarCatalog {
    /// The `ITEM` chunks in file order. The engine loads a prefix of them, so keep them sorted
    /// by [`Star::magnitude`].
    pub stars: Vec<Star>,
    /// Chunks other than `ITEM`, in file order. The engine skips them, they are written after
    /// the stars. The game's catalog has none.
    pub unknown_chunks: Vec<StarsChunk>,
}

/// A star, the contents of an `ITEM` chunk. Positions are equatorial coordinates, epoch and
/// equinox J2000 (Hipparcos' own epoch is J1991.25), the other values are the catalogue's.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Star {
    /// In hours, `0` to `24`.
    pub right_ascension: f32,
    /// In degrees, `-90` to `90`.
    pub declination: f32,
    /// Proper motion in right ascension, `μα cos δ`, in milliarcseconds per year.
    pub proper_motion_ra: f32,
    /// Proper motion in declination in milliarcseconds per year.
    pub proper_motion_dec: f32,
    /// In milliarcseconds. Negative for some faint stars, as in the catalogue.
    pub parallax: f32,
    /// Apparent visual (Johnson V) magnitude, lower is brighter.
    pub magnitude: f32,
    /// `0xAARRGGBB`, so the bytes in the file are blue, green, red, alpha. Alpha is `255` in
    /// the game's catalog.
    pub color: u32,
    /// Hipparcos catalogue number (HIP). Several entries share one for stars with more than
    /// one name, e.g. `CAPELLA`, `CAPELLA A` and `CAPELLA B`.
    pub hip: u32,
    /// Upper case proper name, empty for most stars. Not used by the engine.
    pub name: String,
}

impl Star {
    /// Brightness relative to a star of magnitude 0, `10^(-0.4 * magnitude)`. The engine
    /// computes this on load.
    #[must_use]
    pub fn brightness(&self) -> f32 {
        10f32.powf(-0.4 * self.magnitude)
    }

    /// [`Star::color`] as `[red, green, blue, alpha]`.
    #[must_use]
    pub const fn rgba(&self) -> [u8; 4] {
        let [b, g, r, a] = self.color.to_le_bytes();
        [r, g, b, a]
    }

    /// Sets [`Star::color`] from `[red, green, blue, alpha]`.
    pub const fn set_rgba(&mut self, [r, g, b, a]: [u8; 4]) {
        self.color = u32::from_le_bytes([b, g, r, a]);
    }

    fn parse(data: &[u8], index: usize) -> Result<Self, StarsError> {
        if data.len() < ITEM_FIXED_SIZE {
            return Err(StarsError::InvalidItem(index));
        }
        let u32_at = |o: usize| u32::from_le_bytes(bytes4(&data[o..]));
        let f32_at = |o: usize| f32::from_bits(u32_at(o));
        let name_len = u32_at(32) as usize;
        if name_len == 0 || data.len() - ITEM_FIXED_SIZE != name_len {
            return Err(StarsError::InvalidItem(index));
        }
        let name = match &data[ITEM_FIXED_SIZE..] {
            [name @ .., 0] if !name.contains(&0) => std::str::from_utf8(name).ok(),
            _ => None,
        }
        .ok_or(StarsError::InvalidName(index))?;
        Ok(Self {
            right_ascension: f32_at(0),
            declination: f32_at(4),
            proper_motion_ra: f32_at(8),
            proper_motion_dec: f32_at(12),
            parallax: f32_at(16),
            magnitude: f32_at(20),
            color: u32_at(24),
            hip: u32_at(28),
            name: name.to_owned(),
        })
    }

    fn write_item(&self, index: usize, out: &mut Vec<u8>) -> Result<(), StarsError> {
        if self.name.contains('\0') {
            return Err(StarsError::InvalidName(index));
        }
        let size = ITEM_FIXED_SIZE + self.name.len() + 1;
        let size = u32::try_from(size).map_err(|_| StarsError::TooLarge)?;
        out.extend_from_slice(ITEM_TAG);
        out.extend_from_slice(&size.to_be_bytes());
        for value in [
            self.right_ascension,
            self.declination,
            self.proper_motion_ra,
            self.proper_motion_dec,
            self.parallax,
            self.magnitude,
        ] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&self.color.to_le_bytes());
        out.extend_from_slice(&self.hip.to_le_bytes());
        out.extend_from_slice(&(size - ITEM_FIXED_SIZE as u32).to_le_bytes());
        out.extend_from_slice(self.name.as_bytes());
        out.push(0);
        Ok(())
    }
}

/// A chunk of the `FORM` other than `ITEM`, with a big endian tag and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarsChunk {
    pub tag: [u8; 4],
    pub data: Vec<u8>,
}

impl StarCatalog {
    /// Reads a catalog. The star count after the form type has to match the `ITEM` chunks.
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let form = FormHeader::read_form(input)?;
        if &form.form_type != STARS_FORM_TYPE {
            return Err(StarsError::UnknownFormType(form.form_type_lossy()).into());
        }
        let body_len = (form.size as usize)
            .checked_sub(4)
            .ok_or(StarsError::Truncated)?;
        let mut body = Vec::new();
        input.take(body_len as u64).read_to_end(&mut body)?;
        if body.len() != body_len || body.len() < 4 {
            return Err(StarsError::Truncated.into());
        }

        let count = u32::from_le_bytes(bytes4(&body));
        let mut stars = Vec::with_capacity((count as usize).min(body.len() / 45));
        let mut unknown_chunks = Vec::new();
        let mut rest = &body[4..];
        while !rest.is_empty() {
            let (tag, data);
            (tag, data, rest) = split_chunk(rest).ok_or(StarsError::Truncated)?;
            if &tag == ITEM_TAG {
                stars.push(Star::parse(data, stars.len())?);
            } else {
                unknown_chunks.push(StarsChunk {
                    tag,
                    data: data.to_vec(),
                });
            }
        }
        if stars.len() != count as usize {
            return Err(StarsError::CountMismatch {
                header: count,
                items: stars.len(),
            }
            .into());
        }
        Ok(Self {
            stars,
            unknown_chunks,
        })
    }

    /// Writes the catalog at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let count = u32::try_from(self.stars.len()).map_err(|_| StarsError::TooLarge)?;
        let mut body = Vec::with_capacity(4 + self.stars.len() * 45);
        body.extend_from_slice(&count.to_le_bytes());
        for (index, star) in self.stars.iter().enumerate() {
            star.write_item(index, &mut body)?;
        }
        for chunk in &self.unknown_chunks {
            let size = u32::try_from(chunk.data.len()).map_err(|_| StarsError::TooLarge)?;
            body.extend_from_slice(&chunk.tag);
            body.extend_from_slice(&size.to_be_bytes());
            body.extend_from_slice(&chunk.data);
        }
        FormHeader {
            size: u32::try_from(body.len() + 4).map_err(|_| StarsError::TooLarge)?,
            form_type: *STARS_FORM_TYPE,
        }
        .write(&mut NoSeek::new(&mut *out))?;
        out.write_all(&body)?;
        Ok(())
    }

    /// The first star with this name, ignoring ASCII case.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Star> {
        self.stars
            .iter()
            .find(|s| !s.name.is_empty() && s.name.eq_ignore_ascii_case(name))
    }

    /// Sorts the stars by magnitude, brightest first, like the game's catalog. Stable, so
    /// entries of the same star keep their order.
    pub fn sort_by_magnitude(&mut self) {
        self.stars
            .sort_by(|a, b| a.magnitude.total_cmp(&b.magnitude));
    }
}

/// Splits off the first chunk: tag, contents and the data after it.
fn split_chunk(data: &[u8]) -> Option<([u8; 4], &[u8], &[u8])> {
    let header = data.get(..ChunkHeader::SIZE as usize)?;
    let tag = bytes4(header);
    let size = u32::from_be_bytes(bytes4(&header[4..])) as usize;
    let rest = &data[ChunkHeader::SIZE as usize..];
    (rest.len() >= size).then(|| (tag, &rest[..size], &rest[size..]))
}

/// The first 4 bytes, `data` has to have at least 4.
const fn bytes4(data: &[u8]) -> [u8; 4] {
    [data[0], data[1], data[2], data[3]]
}
