use std::io::{Read, Seek, Write};

use binrw::{BinWrite, io::NoSeek};

use crate::{
    core::iff::FormHeader,
    errors::{AffError, FntError},
};

/// Oldest version the engine reads (`FNT2`).
pub const FNT_MIN_VERSION: u8 = 2;
/// Newest known version (`FNT5`).
pub const FNT_MAX_VERSION: u8 = 5;

/// Enfusion font (`.fnt`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq)]
pub struct Fnt {
    /// The digit of the form type `FNT<version>`, [`FNT_MIN_VERSION`] to [`FNT_MAX_VERSION`].
    pub version: u8,
    pub head: FntHead,
    /// The first 8 bytes of `GLPS`. The writers leave them uninitialized, so they hold leftover
    /// memory (`cdcdcdcd...`, parts of strings, pointers). Kept for byte exact round trips, use
    /// zeros for new fonts.
    pub glyphs_unused: [u8; 8],
    /// The code points of the font, every range gives [`FntRange::count`] consecutive code points
    /// their glyphs. The game's fonts are not always sorted. The Font Editor always adds `U+0020`
    /// and `U+25A1` (the glyph for missing characters), distance field fonts need both.
    pub ranges: Vec<FntRange>,
    /// One glyph per code point of [`Fnt::ranges`], in the same order.
    pub glyphs: Vec<FntGlyph>,
    /// Colour glyphs, in file order.
    pub color_glyphs: Vec<FntColorGlyph>,
    pub kerning: Vec<FntKerning>,
    /// The chunks of the file in file order. The writer writes exactly these, so it has to
    /// include [`FntChunk::Layers`] and [`FntChunk::Kerning`] when they have data. The game's
    /// `FNT3` fonts have `GLPS TCRD KERN HEAD`, the Font Editor writes
    /// [`Fnt::editor_chunk_order`].
    pub chunk_order: Vec<FntChunk>,
}

/// A chunk of an [`Fnt`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FntChunk {
    /// `HEAD`, [`Fnt::head`].
    Head,
    /// `GLPS`, [`Fnt::ranges`].
    Glyphs,
    /// `TCRD`, [`Fnt::glyphs`].
    TexCoords,
    /// `LYRS`, [`Fnt::color_glyphs`].
    Layers,
    /// `KERN`, [`Fnt::kerning`].
    Kerning,
}

impl FntChunk {
    #[must_use]
    pub const fn tag(self) -> &'static [u8; 4] {
        match self {
            Self::Head => b"HEAD",
            Self::Glyphs => b"GLPS",
            Self::TexCoords => b"TCRD",
            Self::Layers => b"LYRS",
            Self::Kerning => b"KERN",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Head => "HEAD",
            Self::Glyphs => "GLPS",
            Self::TexCoords => "TCRD",
            Self::Layers => "LYRS",
            Self::Kerning => "KERN",
        }
    }

    fn from_tag(tag: [u8; 4]) -> Option<Self> {
        [
            Self::Head,
            Self::Glyphs,
            Self::TexCoords,
            Self::Layers,
            Self::Kerning,
        ]
        .into_iter()
        .find(|c| *c.tag() == tag)
    }
}

/// Contents of the `HEAD` chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct FntHead {
    /// The font's name: the file name the Font Editor saved it under, without extension. The
    /// engine reads at most 254 bytes.
    pub name: String,
    /// Font size in pixels the atlas was rendered at. The engine does not read it.
    pub size: u32,
    /// Bitmap fonts only: pixels to move the text down (up if negative), added to the top of
    /// every line when drawing, not scaled with the text size and ignored when measuring text.
    /// Distance field fonts don't read it. `0` in all fonts, the Font Editor always writes `0`.
    pub y_offset: i32,
    /// The rest of the chunk, which exists from version 4.
    pub metrics: Option<FntMetrics>,
}

/// The part of [`FntHead`] that exists from version 4. Sizes are pixels at [`FntHead::size`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FntMetrics {
    pub font_type: FntType,
    /// Edge length of the atlas cells: the largest glyph width or height plus
    /// [`FntMetrics::sdf_range`]. Every glyph is centered in its cell, see [`Fnt::atlas_rect`].
    pub cell_size: u32,
    /// Height of `X` above the baseline.
    pub cap_height: f32,
    /// Vertical pixels per em of the rendered font (`FreeType` `y_ppem`), [`FntHead::size`] in the
    /// game's fonts. The engine scales from this size.
    pub y_ppem: f32,
    /// Distance field range in pixels, the "SDF radius" of the Font Editor.
    pub sdf_range: u16,
    /// The Font Editor's Bold checkbox. It doesn't change the atlas or the glyphs, the engine
    /// applies the style at runtime.
    pub bold: bool,
    /// The Font Editor's Italic checkbox, like [`FntMetrics::bold`].
    pub italic: bool,
    /// Horizontal pixels per em (`FreeType` `x_ppem`).
    pub x_ppem: f32,
}

/// The kind of atlas, [`FntMetrics::font_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FntType {
    /// Plain glyph images.
    Bitmap = 0,
    /// Single channel signed distance field.
    Sdf = 1,
    /// Multi channel signed distance field.
    Msdf = 2,
}

impl FntType {
    const fn from_u8(value: u8) -> Result<Self, FntError> {
        match value {
            0 => Ok(Self::Bitmap),
            1 => Ok(Self::Sdf),
            2 => Ok(Self::Msdf),
            other => Err(FntError::UnknownFontType(other)),
        }
    }
}

/// Consecutive code points, an entry of [`Fnt::ranges`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FntRange {
    /// The first code point. Below version 5 at most `0xFFFF`.
    pub first: u32,
    pub count: u16,
    /// Padding after the count from version 5 on, uninitialized memory like
    /// [`Fnt::glyphs_unused`]. `0` below version 5.
    pub padding: u16,
}

/// Where a glyph is in the atlas and how to place it, an entry of [`Fnt::glyphs`]. Atlas
/// positions are pixels from the top left.
///
/// Distance field fonts: the glyph's box is `width` x `height`, centered in the atlas cell at
/// `x`, `y` (see [`Fnt::atlas_rect`]). `offset_x` is the distance from the pen position to the
/// left of the box, `offset_y` from the baseline up to its top, `advance` how far the pen moves.
///
/// Bitmap fonts: the glyph is the `width` x `height` rectangle at `x`, `y`. `offset_x` is the
/// space before it and `offset_y` the space after it (Windows' ABC widths A and C), so the pen
/// moves `offset_x + width + offset_y`. `advance` is not used. Version 2 has neither offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FntGlyph {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub offset_x: i16,
    pub offset_y: i16,
    /// From version 4 on.
    pub advance: i16,
}

/// A colour glyph, an entry of the `LYRS` chunk: a code point drawn as coloured layers.
///
/// The Font Editor writes them for the glyphs of the TrueType font's `COLR` table (emoji). None
/// of the game's fonts has any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FntColorGlyph {
    pub code_point: u32,
    /// Bottom first, from the `COLR` layer records.
    pub layers: Vec<FntLayer>,
}

/// A layer of an [`FntColorGlyph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FntLayer {
    /// The glyph with the layer's shape, a code point of this font that is not Unicode
    /// ([`FntLayer::FIRST_CODE_POINT`] or higher). The Font Editor numbers them after the
    /// TrueType glyph id (`0x110000 + id - 1`), layers with the same shape share one. Their
    /// [`FntGlyph::advance`] is 0.
    pub code_point: u32,
    /// `0xAARRGGBB`, the layer's colour from the font's `CPAL` palette.
    pub color: u32,
}

impl FntLayer {
    /// The first code point after Unicode, where the Font Editor puts layer glyphs.
    pub const FIRST_CODE_POINT: u32 = 0x11_0000;
}

/// Kerning of a pair of code points, an entry of [`Fnt::kerning`].
///
/// The Font Editor takes the pairs of the TrueType font's legacy `kern` table (not `GPOS`, so e.g. Roboto has none) between code
/// points of the font, without the ones that are `0` in the table, and rounds them to pixels at
/// the font size, which can give `0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FntKerning {
    pub first: u16,
    pub second: u16,
    /// Pixels to add to the advance of `first` when `second` follows it.
    pub amount: i32,
}

impl Fnt {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let form = FormHeader::read_form(input)?;
        let version = match form.form_type {
            [b'F', b'N', b'T', digit]
                if (b'0' + FNT_MIN_VERSION..=b'0' + FNT_MAX_VERSION).contains(&digit) =>
            {
                digit - b'0'
            }
            _ => return Err(FntError::UnknownFormType(form.form_type_lossy()).into()),
        };
        let body_size = u64::from(form.size.saturating_sub(4));
        let mut body = Vec::new();
        input.take(body_size).read_to_end(&mut body)?;
        if body.len() as u64 != body_size {
            return Err(FntError::Truncated.into());
        }
        Ok(Self::parse(version, &body)?)
    }

    fn parse(version: u8, body: &[u8]) -> Result<Self, FntError> {
        let mut chunks: [Option<&[u8]>; 5] = [None; 5];
        let mut chunk_order = Vec::new();
        let mut pos = 0;
        while pos < body.len() {
            let header = body.get(pos..pos + 8).ok_or(FntError::Truncated)?;
            let tag: [u8; 4] = header[..4].try_into().unwrap_or_default();
            let size = u32::from_be_bytes(header[4..].try_into().unwrap_or_default()) as usize;
            let data = body
                .get(pos + 8..)
                .and_then(|rest| rest.get(..size))
                .ok_or(FntError::Truncated)?;
            let tag_name = || String::from_utf8_lossy(&tag).into_owned();
            let chunk =
                FntChunk::from_tag(tag).ok_or_else(|| FntError::UnknownChunk(tag_name()))?;
            let slot = &mut chunks[chunk as usize];
            if slot.is_some() {
                return Err(FntError::DuplicateChunk(tag_name()));
            }
            *slot = Some(data);
            chunk_order.push(chunk);
            pos += 8 + size;
        }
        let required = |chunk: FntChunk| {
            chunks[chunk as usize].ok_or_else(|| FntError::ChunkMissing(chunk.name()))
        };

        let head = parse_head(version, required(FntChunk::Head)?)?;
        let (glyphs_unused, glyph_count, ranges) =
            parse_ranges(version, required(FntChunk::Glyphs)?)?;
        let glyphs = parse_glyphs(version, required(FntChunk::TexCoords)?)?;
        let range_glyphs: usize = ranges.iter().map(|r| usize::from(r.count)).sum();
        if glyph_count as usize != range_glyphs {
            return Err(FntError::InvalidChunk("GLPS"));
        }
        if glyphs.len() != range_glyphs {
            return Err(FntError::InvalidChunk("TCRD"));
        }
        let color_glyphs = chunks[FntChunk::Layers as usize]
            .map(parse_layers)
            .transpose()?
            .unwrap_or_default();
        let kerning = chunks[FntChunk::Kerning as usize]
            .map(parse_kerning)
            .transpose()?
            .unwrap_or_default();

        Ok(Self {
            version,
            head,
            glyphs_unused,
            ranges,
            glyphs,
            color_glyphs,
            kerning,
            chunk_order,
        })
    }

    /// Writes the font at the current position of `out`, the chunks in [`Fnt::chunk_order`].
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        if !(FNT_MIN_VERSION..=FNT_MAX_VERSION).contains(&self.version) {
            return Err(FntError::Unwritable("version").into());
        }
        for chunk in [FntChunk::Head, FntChunk::Glyphs, FntChunk::TexCoords] {
            if !self.chunk_order.contains(&chunk) {
                return Err(FntError::ChunkMissing(chunk.name()).into());
            }
        }
        for (chunk, has_data) in [
            (FntChunk::Layers, !self.color_glyphs.is_empty()),
            (FntChunk::Kerning, !self.kerning.is_empty()),
        ] {
            if has_data && !self.chunk_order.contains(&chunk) {
                return Err(FntError::Unwritable(chunk.name()).into());
            }
        }

        let mut body = Vec::new();
        for (i, &chunk) in self.chunk_order.iter().enumerate() {
            if self.chunk_order[..i].contains(&chunk) {
                return Err(FntError::DuplicateChunk(chunk.name().to_owned()).into());
            }
            let data = match chunk {
                FntChunk::Head => self.head_data()?,
                FntChunk::Glyphs => self.ranges_data()?,
                FntChunk::TexCoords => self.glyphs_data()?,
                FntChunk::Layers => self.layers_data()?,
                FntChunk::Kerning => self.kerning_data(),
            };
            body.extend_from_slice(chunk.tag());
            body.extend_from_slice(&len_u32(data.len())?.to_be_bytes());
            body.extend_from_slice(&data);
        }

        FormHeader {
            size: len_u32(body.len() + 4)?,
            form_type: [b'F', b'N', b'T', b'0' + self.version],
        }
        .write(&mut NoSeek::new(&mut *out))?;
        out.write_all(&body)?;
        Ok(())
    }

    /// The order the Font Editor writes the chunks in: `HEAD GLPS TCRD`, `LYRS` if there are
    /// colour glyphs and the version is not 3, `KERN`.
    #[must_use]
    pub fn editor_chunk_order(&self) -> Vec<FntChunk> {
        let mut order = vec![FntChunk::Head, FntChunk::Glyphs, FntChunk::TexCoords];
        if !self.color_glyphs.is_empty() && self.version != 3 {
            order.push(FntChunk::Layers);
        }
        order.push(FntChunk::Kerning);
        order
    }

    /// The code points of [`Fnt::glyphs`], in the same order.
    pub fn code_points(&self) -> impl Iterator<Item = u32> + '_ {
        self.ranges
            .iter()
            .flat_map(|r| (0..u32::from(r.count)).map(move |i| r.first.wrapping_add(i)))
    }

    /// The glyph of a code point. If ranges overlap, the first one wins.
    #[must_use]
    pub fn glyph(&self, code_point: u32) -> Option<&FntGlyph> {
        let mut index = 0;
        for range in &self.ranges {
            let offset = code_point.wrapping_sub(range.first);
            if offset < u32::from(range.count) {
                return self.glyphs.get(index + offset as usize);
            }
            index += usize::from(range.count);
        }
        None
    }

    /// The kerning of a pair of code points, `None` without a [`FntKerning`] for them.
    #[must_use]
    pub fn kerning(&self, first: u32, second: u32) -> Option<i32> {
        let (first, second) = (u16::try_from(first).ok()?, u16::try_from(second).ok()?);
        self.kerning
            .iter()
            .find(|k| k.first == first && k.second == second)
            .map(|k| k.amount)
    }

    /// The layers of a colour glyph, `None` if the code point is drawn with its glyph only.
    #[must_use]
    pub fn color_glyph(&self, code_point: u32) -> Option<&FntColorGlyph> {
        self.color_glyphs
            .iter()
            .find(|c| c.code_point == code_point)
    }

    /// The pixels of a glyph in the atlas as `[x0, y0, x1, y1]`, the way the engine computes
    /// them. Distance field fonts center the glyph in its [`FntMetrics::cell_size`] cell at
    /// [`FntGlyph::x`], [`FntGlyph::y`].
    #[must_use]
    pub fn atlas_rect(&self, glyph: &FntGlyph) -> [f32; 4] {
        let (width, height) = (f32::from(glyph.width), f32::from(glyph.height));
        let (mut x, mut y) = (f32::from(glyph.x), f32::from(glyph.y));
        if let Some(metrics) = self.head.metrics
            && metrics.font_type != FntType::Bitmap
        {
            let cell = metrics.cell_size as f32;
            x = (cell - width).mul_add(0.5, x);
            y = (cell - height).mul_add(0.5, y);
        }
        [x, y, x + width, y + height]
    }

    fn head_data(&self) -> Result<Vec<u8>, FntError> {
        let head = &self.head;
        if head.metrics.is_some() != (self.version >= 4) {
            return Err(FntError::Unwritable("HEAD"));
        }
        let mut out = Vec::new();
        out.extend_from_slice(&len_u32(head.name.len() + 1)?.to_le_bytes());
        out.extend_from_slice(head.name.as_bytes());
        out.push(0);
        out.extend_from_slice(&head.size.to_le_bytes());
        out.extend_from_slice(&head.y_offset.to_le_bytes());
        if let Some(m) = head.metrics {
            out.push(m.font_type as u8);
            out.extend_from_slice(&m.cell_size.to_le_bytes());
            out.extend_from_slice(&m.cap_height.to_le_bytes());
            out.extend_from_slice(&m.y_ppem.to_le_bytes());
            out.extend_from_slice(&m.sdf_range.to_le_bytes());
            out.push(m.bold.into());
            out.push(m.italic.into());
            out.extend_from_slice(&m.x_ppem.to_le_bytes());
        }
        Ok(out)
    }

    fn ranges_data(&self) -> Result<Vec<u8>, FntError> {
        let glyph_count: usize = self.ranges.iter().map(|r| usize::from(r.count)).sum();
        let mut out = Vec::new();
        out.extend_from_slice(&self.glyphs_unused);
        out.extend_from_slice(&len_u32(glyph_count)?.to_le_bytes());
        out.extend_from_slice(&len_u32(self.ranges.len())?.to_le_bytes());
        for range in &self.ranges {
            if self.version >= 5 {
                out.extend_from_slice(&range.first.to_le_bytes());
                out.extend_from_slice(&range.count.to_le_bytes());
                out.extend_from_slice(&range.padding.to_le_bytes());
            } else {
                let first = u16::try_from(range.first)
                    .ok()
                    .filter(|_| range.padding == 0)
                    .ok_or(FntError::Unwritable("GLPS"))?;
                out.extend_from_slice(&first.to_le_bytes());
                out.extend_from_slice(&range.count.to_le_bytes());
            }
        }
        Ok(out)
    }

    fn glyphs_data(&self) -> Result<Vec<u8>, FntError> {
        let glyph_count: usize = self.ranges.iter().map(|r| usize::from(r.count)).sum();
        if self.glyphs.len() != glyph_count {
            return Err(FntError::Unwritable("TCRD"));
        }
        let mut out = Vec::with_capacity(self.glyphs.len() * glyph_size(self.version));
        for g in &self.glyphs {
            if (self.version < 3 && (g.offset_x, g.offset_y) != (0, 0))
                || (self.version < 4 && g.advance != 0)
            {
                return Err(FntError::Unwritable("TCRD"));
            }
            for value in [g.x, g.y, g.width, g.height] {
                out.extend_from_slice(&value.to_le_bytes());
            }
            if self.version >= 3 {
                out.extend_from_slice(&g.offset_x.to_le_bytes());
                out.extend_from_slice(&g.offset_y.to_le_bytes());
            }
            if self.version >= 4 {
                out.extend_from_slice(&g.advance.to_le_bytes());
            }
        }
        Ok(out)
    }

    fn layers_data(&self) -> Result<Vec<u8>, FntError> {
        let mut out = Vec::new();
        out.extend_from_slice(&len_u32(self.color_glyphs.len())?.to_le_bytes());
        for entry in &self.color_glyphs {
            out.extend_from_slice(&len_u32(8 + entry.layers.len() * 8)?.to_le_bytes());
            out.extend_from_slice(&entry.code_point.to_le_bytes());
            out.extend_from_slice(&len_u32(entry.layers.len())?.to_le_bytes());
            for layer in &entry.layers {
                out.extend_from_slice(&layer.code_point.to_le_bytes());
                out.extend_from_slice(&layer.color.to_le_bytes());
            }
        }
        Ok(out)
    }

    fn kerning_data(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.kerning.len() * 8);
        for k in &self.kerning {
            out.extend_from_slice(&k.first.to_le_bytes());
            out.extend_from_slice(&k.second.to_le_bytes());
            out.extend_from_slice(&k.amount.to_le_bytes());
        }
        out
    }
}

const fn glyph_size(version: u8) -> usize {
    match version {
        ..=2 => 8,
        3 => 12,
        _ => 14,
    }
}

fn parse_head(version: u8, data: &[u8]) -> Result<FntHead, FntError> {
    let mut r = ChunkReader::new(data, "HEAD");
    let name = r.string()?;
    let size = r.u32()?;
    let y_offset = r.i32()?;
    let metrics = if version >= 4 {
        Some(FntMetrics {
            font_type: FntType::from_u8(r.u8()?)?,
            cell_size: r.u32()?,
            cap_height: r.f32()?,
            y_ppem: r.f32()?,
            sdf_range: r.u16()?,
            bold: r.bool()?,
            italic: r.bool()?,
            x_ppem: r.f32()?,
        })
    } else {
        None
    };
    r.finish()?;
    Ok(FntHead {
        name,
        size,
        y_offset,
        metrics,
    })
}

fn parse_ranges(version: u8, data: &[u8]) -> Result<([u8; 8], u32, Vec<FntRange>), FntError> {
    let mut r = ChunkReader::new(data, "GLPS");
    let unused = r.array()?;
    let glyph_count = r.u32()?;
    let count = r.u32()? as usize;
    let range_size = if version >= 5 { 8 } else { 4 };
    // The count is untrusted, don't allocate more than the data can hold.
    let mut ranges = Vec::with_capacity(count.min(data.len() / range_size));
    for _ in 0..count {
        ranges.push(if version >= 5 {
            FntRange {
                first: r.u32()?,
                count: r.u16()?,
                padding: r.u16()?,
            }
        } else {
            FntRange {
                first: r.u16()?.into(),
                count: r.u16()?,
                padding: 0,
            }
        });
    }
    r.finish()?;
    Ok((unused, glyph_count, ranges))
}

fn parse_glyphs(version: u8, data: &[u8]) -> Result<Vec<FntGlyph>, FntError> {
    let size = glyph_size(version);
    if !data.len().is_multiple_of(size) {
        return Err(FntError::InvalidChunk("TCRD"));
    }
    let mut r = ChunkReader::new(data, "TCRD");
    let mut glyphs = Vec::with_capacity(data.len() / size);
    for _ in 0..data.len() / size {
        let mut glyph = FntGlyph {
            x: r.u16()?,
            y: r.u16()?,
            width: r.u16()?,
            height: r.u16()?,
            ..FntGlyph::default()
        };
        if version >= 3 {
            glyph.offset_x = r.i16()?;
            glyph.offset_y = r.i16()?;
        }
        if version >= 4 {
            glyph.advance = r.i16()?;
        }
        glyphs.push(glyph);
    }
    r.finish()?;
    Ok(glyphs)
}

fn parse_layers(data: &[u8]) -> Result<Vec<FntColorGlyph>, FntError> {
    let mut r = ChunkReader::new(data, "LYRS");
    let count = r.u32()? as usize;
    let mut entries = Vec::with_capacity(count.min(data.len() / 12));
    for _ in 0..count {
        let size = r.u32()? as usize;
        let mut entry = ChunkReader::new(r.bytes(size)?, "LYRS");
        let code_point = entry.u32()?;
        let layer_count = entry.u32()? as usize;
        let mut layers = Vec::with_capacity(layer_count.min(size / 8));
        for _ in 0..layer_count {
            layers.push(FntLayer {
                code_point: entry.u32()?,
                color: entry.u32()?,
            });
        }
        entry.finish()?;
        entries.push(FntColorGlyph { code_point, layers });
    }
    r.finish()?;
    Ok(entries)
}

fn parse_kerning(data: &[u8]) -> Result<Vec<FntKerning>, FntError> {
    if !data.len().is_multiple_of(8) {
        return Err(FntError::InvalidChunk("KERN"));
    }
    let mut r = ChunkReader::new(data, "KERN");
    let mut pairs = Vec::with_capacity(data.len() / 8);
    for _ in 0..data.len() / 8 {
        pairs.push(FntKerning {
            first: r.u16()?,
            second: r.u16()?,
            amount: r.i32()?,
        });
    }
    Ok(pairs)
}

/// Little endian reads from a chunk's contents, failing with [`FntError::InvalidChunk`].
struct ChunkReader<'a> {
    data: &'a [u8],
    pos: usize,
    tag: &'static str,
}

impl<'a> ChunkReader<'a> {
    const fn new(data: &'a [u8], tag: &'static str) -> Self {
        Self { data, pos: 0, tag }
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], FntError> {
        let bytes = self
            .pos
            .checked_add(len)
            .and_then(|end| self.data.get(self.pos..end))
            .ok_or(FntError::InvalidChunk(self.tag))?;
        self.pos += len;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], FntError> {
        let bytes = self.bytes(N)?;
        Ok(bytes.try_into().unwrap_or([0; N]))
    }

    fn u8(&mut self) -> Result<u8, FntError> {
        Ok(self.array::<1>()?[0])
    }

    fn bool(&mut self) -> Result<bool, FntError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(FntError::InvalidChunk(self.tag)),
        }
    }

    fn u16(&mut self) -> Result<u16, FntError> {
        self.array().map(u16::from_le_bytes)
    }

    fn i16(&mut self) -> Result<i16, FntError> {
        self.array().map(i16::from_le_bytes)
    }

    fn u32(&mut self) -> Result<u32, FntError> {
        self.array().map(u32::from_le_bytes)
    }

    fn i32(&mut self) -> Result<i32, FntError> {
        self.array().map(i32::from_le_bytes)
    }

    fn f32(&mut self) -> Result<f32, FntError> {
        self.array().map(f32::from_le_bytes)
    }

    /// A `u32` length including the NUL, then the bytes and the NUL.
    fn string(&mut self) -> Result<String, FntError> {
        let len = self.u32()? as usize;
        let Some((&0, text)) = self.bytes(len)?.split_last() else {
            return Err(FntError::InvalidChunk(self.tag));
        };
        String::from_utf8(text.to_vec()).map_err(|_| FntError::InvalidChunk(self.tag))
    }

    const fn finish(self) -> Result<(), FntError> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(FntError::InvalidChunk(self.tag))
        }
    }
}

fn len_u32(len: usize) -> Result<u32, FntError> {
    u32::try_from(len).map_err(|_| FntError::TooLarge)
}
