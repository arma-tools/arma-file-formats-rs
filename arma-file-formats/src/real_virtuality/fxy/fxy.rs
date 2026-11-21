use std::io::{Read, Write};

use crate::errors::{AffError, FxyError};

/// The signature of a `.fxy`.
pub const FXY_SIGNATURE: &[u8; 4] = b"BIFo";
/// Version 1, a flat list of [`FxyCell`]s.
pub const FXY_VERSION_1: u32 = 0x101;
/// Version 2, [`FxyPage`]s with kerning and glyph offsets.
pub const FXY_VERSION_2: u32 = 0x102;
/// [`FxyGlyph::x_offset`] and [`FxyGlyph::y_offset`] that `FontToTGA` writes for glyphs without
/// pixels, like the space.
pub const EMPTY_GLYPH_OFFSET: i32 = -10;
/// Version 1 stores characters minus this.
const V1_CODE_BASE: u32 = 0x20;

/// A font, see the [module docs](super).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fxy {
    /// Version 1.
    V1 { cells: Vec<FxyCell> },
    /// Version 2.
    V2 { pages: Vec<FxyPage> },
}

/// A glyph of a version 1 font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxyCell {
    /// The character (UTF-16 code unit), stored minus `0x20`.
    pub code: u32,
    /// Texture page, from `1`.
    pub page: u16,
    /// Left of the rectangle on the page, at the pen position.
    pub x: u16,
    /// Top of the rectangle on the page, at the top of the line.
    pub y: u16,
    /// Can be one more than [`FxyCell::advance`].
    pub width: u16,
    /// From the top of the line to the glyph's lowest pixel.
    pub height: u16,
    /// How far the pen moves.
    pub advance: u16,
}

/// A texture page of a version 2 font.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FxyPage {
    /// Texture page, from `1`. `FontToTGA` counts them up in file order.
    pub page: u16,
    /// Line height in pixels, the same on every page.
    pub height: i32,
    /// From the top of the line to the baseline, the same on every page.
    pub ascent: i32,
    /// Pairs whose first character is on this page.
    pub kerning: Vec<FxyKerning>,
    pub glyphs: Vec<FxyGlyph>,
}

/// A kerning pair of a version 2 font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxyKerning {
    pub first: u16,
    pub second: u16,
    /// Pixels added to the advance of `first` when `second` follows.
    pub amount: i32,
}

/// A glyph of a version 2 font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxyGlyph {
    /// The character (UTF-16 code unit).
    pub code: u16,
    /// Left of the glyph's pixels on the page.
    pub x: u16,
    /// Top of the glyph's pixels on the page.
    pub y: u16,
    pub width: u16,
    pub height: u16,
    /// From the pen position to the left of the pixels, [`EMPTY_GLYPH_OFFSET`] without pixels.
    pub x_offset: i32,
    /// From the top of the line to the top of the pixels, [`EMPTY_GLYPH_OFFSET`] without pixels.
    pub y_offset: i32,
    /// How far the pen moves.
    pub advance: i32,
}

/// Where a character's glyph is and how to place it, the same for both versions, see
/// [`Fxy::glyph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxyLetter {
    /// Texture page, from `1`.
    pub page: u16,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    /// From the pen position to the left of the rectangle, `0` in version 1.
    pub x_offset: i32,
    /// From the top of the line to the top of the rectangle, `0` in version 1.
    pub y_offset: i32,
    pub advance: i32,
}

impl Fxy {
    pub fn from<I: Read>(input: &mut I) -> Result<Self, AffError> {
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        Ok(Self::parse(&bytes)?)
    }

    fn parse(bytes: &[u8]) -> Result<Self, FxyError> {
        let mut input = Reader { bytes, pos: 0 };
        let signature = input.take(4)?;
        if signature != FXY_SIGNATURE {
            return Err(FxyError::UnknownSignature(
                String::from_utf8_lossy(signature).into_owned(),
            ));
        }
        match input.u32()? {
            FXY_VERSION_1 => {
                let mut cells = Vec::with_capacity(input.remaining() / 14);
                while input.remaining() != 0 {
                    cells.push(FxyCell {
                        code: u32::from(input.u16()?) + V1_CODE_BASE,
                        page: input.u16()?,
                        x: input.u16()?,
                        y: input.u16()?,
                        width: input.u16()?,
                        height: input.u16()?,
                        advance: input.u16()?,
                    });
                }
                Ok(Self::V1 { cells })
            }
            FXY_VERSION_2 => {
                let mut pages = Vec::new();
                while input.remaining() != 0 {
                    pages.push(FxyPage::parse(&mut input)?);
                }
                Ok(Self::V2 { pages })
            }
            version => Err(FxyError::UnsupportedVersion(version)),
        }
    }

    /// Writes the font at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let mut bytes = Vec::new();
        bytes.extend(FXY_SIGNATURE);
        bytes.extend(self.version().to_le_bytes());
        match self {
            Self::V1 { cells } => {
                for cell in cells {
                    let code = cell
                        .code
                        .checked_sub(V1_CODE_BASE)
                        .and_then(|code| u16::try_from(code).ok())
                        .ok_or(FxyError::InvalidCode(cell.code))?;
                    for value in [
                        code,
                        cell.page,
                        cell.x,
                        cell.y,
                        cell.width,
                        cell.height,
                        cell.advance,
                    ] {
                        bytes.extend(value.to_le_bytes());
                    }
                }
            }
            Self::V2 { pages } => {
                for page in pages {
                    page.write(&mut bytes)?;
                }
            }
        }
        out.write_all(&bytes)?;
        Ok(())
    }

    /// [`FXY_VERSION_1`] or [`FXY_VERSION_2`].
    #[must_use]
    pub const fn version(&self) -> u32 {
        match self {
            Self::V1 { .. } => FXY_VERSION_1,
            Self::V2 { .. } => FXY_VERSION_2,
        }
    }

    /// The first glyph of a character (UTF-16 code unit).
    #[must_use]
    pub fn glyph(&self, code: u32) -> Option<FxyLetter> {
        match self {
            Self::V1 { cells } => cells.iter().find(|c| c.code == code).map(|c| FxyLetter {
                page: c.page,
                x: c.x,
                y: c.y,
                width: c.width,
                height: c.height,
                x_offset: 0,
                y_offset: 0,
                advance: i32::from(c.advance),
            }),
            Self::V2 { pages } => pages.iter().find_map(|page| {
                page.glyphs
                    .iter()
                    .find(|g| u32::from(g.code) == code)
                    .map(|g| FxyLetter {
                        page: page.page,
                        x: g.x,
                        y: g.y,
                        width: g.width,
                        height: g.height,
                        x_offset: g.x_offset,
                        y_offset: g.y_offset,
                        advance: g.advance,
                    })
            }),
        }
    }

    /// The kerning of `first` followed by `second`, `0` without a pair and in version 1.
    #[must_use]
    pub fn kerning(&self, first: u32, second: u32) -> i32 {
        match self {
            Self::V1 { .. } => 0,
            Self::V2 { pages } => pages
                .iter()
                .flat_map(|page| &page.kerning)
                .find(|k| u32::from(k.first) == first && u32::from(k.second) == second)
                .map_or(0, |k| k.amount),
        }
    }

    /// Line height and ascent of a version 2 font (from its first page).
    #[must_use]
    pub fn metrics(&self) -> Option<(i32, i32)> {
        match self {
            Self::V1 { .. } => None,
            Self::V2 { pages } => pages.first().map(|p| (p.height, p.ascent)),
        }
    }

    /// The texture of a page next to the font: `Fonts\Purista\Purista10.fxy` and page `1` give
    /// `Fonts\Purista\Purista10-01.paa`. The extension is dropped when it is `.fxy` (any case).
    #[must_use]
    pub fn texture_path(fxy_path: &str, page: u16) -> String {
        let base = fxy_path
            .len()
            .checked_sub(4)
            .filter(|&at| {
                fxy_path
                    .get(at..)
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(".fxy"))
            })
            .map_or(fxy_path, |at| &fxy_path[..at]);
        format!("{base}-{page:02}.paa")
    }
}

impl FxyPage {
    fn parse(input: &mut Reader) -> Result<Self, FxyError> {
        let page = input.u16()?;
        let height = input.i32()?;
        let ascent = input.i32()?;
        let count = input.u32()? as usize;
        let mut kerning = Vec::with_capacity(count.min(input.remaining() / 8));
        for _ in 0..count {
            kerning.push(FxyKerning {
                first: input.u16()?,
                second: input.u16()?,
                amount: input.i32()?,
            });
        }
        let count = input.u32()? as usize;
        let mut glyphs = Vec::with_capacity(count.min(input.remaining() / 22));
        for _ in 0..count {
            glyphs.push(FxyGlyph {
                code: input.u16()?,
                x: input.u16()?,
                y: input.u16()?,
                width: input.u16()?,
                height: input.u16()?,
                x_offset: input.i32()?,
                y_offset: input.i32()?,
                advance: input.i32()?,
            });
        }
        Ok(Self {
            page,
            height,
            ascent,
            kerning,
            glyphs,
        })
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<(), FxyError> {
        out.extend(self.page.to_le_bytes());
        out.extend(self.height.to_le_bytes());
        out.extend(self.ascent.to_le_bytes());
        let count = u32::try_from(self.kerning.len()).map_err(|_| FxyError::TooLarge)?;
        out.extend(count.to_le_bytes());
        for k in &self.kerning {
            out.extend(k.first.to_le_bytes());
            out.extend(k.second.to_le_bytes());
            out.extend(k.amount.to_le_bytes());
        }
        let count = u32::try_from(self.glyphs.len()).map_err(|_| FxyError::TooLarge)?;
        out.extend(count.to_le_bytes());
        for g in &self.glyphs {
            for value in [g.code, g.x, g.y, g.width, g.height] {
                out.extend(value.to_le_bytes());
            }
            for value in [g.x_offset, g.y_offset, g.advance] {
                out.extend(value.to_le_bytes());
            }
        }
        Ok(())
    }
}

/// The data is in memory, so reads only fail at its end.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    const fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], FxyError> {
        let bytes = self
            .pos
            .checked_add(len)
            .and_then(|end| self.bytes.get(self.pos..end))
            .ok_or(FxyError::Truncated)?;
        self.pos += len;
        Ok(bytes)
    }

    fn u16(&mut self) -> Result<u16, FxyError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().unwrap_or_default(),
        ))
    }

    fn u32(&mut self) -> Result<u32, FxyError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }

    fn i32(&mut self) -> Result<i32, FxyError> {
        Ok(i32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }
}
