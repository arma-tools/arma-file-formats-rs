use std::io::{Read, Seek, Write};

use crate::{
    errors::{AffError, TexHeadersError},
    real_virtuality::types::PaaType,
};

/// The signature of a `texHeaders.bin`.
pub const TEXHEADERS_SIGNATURE: &[u8; 4] = b"0DHT";
/// The version Binarize writes, the only one known.
pub const TEXHEADERS_VERSION: u32 = 1;
/// The average color without an `AVGC` tag, as `MAXC` stores colors (`0xAARRGGBB`).
pub const DEFAULT_AVERAGE_COLOR: u32 = 0xff80_2020;
/// The max color without a `MAXC` tag.
pub const DEFAULT_MAX_COLOR: u32 = 0xffff_ffff;

/// A `texHeaders.bin`, see the [module docs](super).
#[derive(Debug, Clone, PartialEq)]
pub struct TexHeaders {
    /// [`TEXHEADERS_VERSION`].
    pub version: u32,
    pub textures: Vec<TexHeader>,
}

/// The header of one texture.
#[derive(Debug, Clone, PartialEq)]
// The format stores these flags as bytes.
#[allow(clippy::struct_excessive_bools)]
pub struct TexHeader {
    /// `1` in every game file.
    pub color_palette_count: u32,
    /// `0` in every game file.
    pub palette_offset: u32,
    /// RGBA, the `AVGC` tag's bytes (stored BGRA) times `1 / 255` in `f32`, without the tag the
    /// [`DEFAULT_AVERAGE_COLOR`].
    pub average_color: [f32; 4],
    /// `0` in every game file, Binarize doesn't fill it.
    pub average_color_packed: u32,
    /// The `MAXC` tag as `u32` (`0xAARRGGBB`), without the tag [`DEFAULT_MAX_COLOR`].
    pub max_color: u32,
    /// `0` in every game file.
    pub clamp_flags: u32,
    /// `0xffffffff` in every game file.
    pub transparent_color: u32,
    /// The texture has a `MAXC` tag.
    pub has_max_color: bool,
    /// Bit `1` of the `FLAG` tag: interpolated alpha.
    pub is_alpha: bool,
    /// Bit `2` of the `FLAG` tag: transparent (key alpha).
    pub is_transparent: bool,
    /// [`TexHeader::is_alpha`] and an average alpha below `128`.
    pub is_alpha_non_opaque: bool,
    pub pixel_format: PixelFormat,
    /// `true` in every game file.
    pub little_endian: bool,
    /// The file is a `.paa`, not a `.pac`.
    pub is_paa: bool,
    /// The path relative to the PBO, lower case, `\` separated.
    pub path: String,
    pub texture_type: TextureType,
    pub mipmaps: Vec<TexHeaderMipmap>,
    /// Size of the texture file in bytes.
    pub file_size: u32,
}

/// A mipmap of a [`TexHeader`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TexHeaderMipmap {
    /// Without the LZO flag (`0x8000`) of the `.paa` mipmap header.
    pub width: u16,
    pub height: u16,
    /// `0` in every game file.
    pub reserved: u16,
    /// [`TexHeader::pixel_format`].
    pub pixel_format: PixelFormat,
    /// `3` in every game file.
    pub unknown: u8,
    /// Position of the mipmap's header (its width) in the texture file.
    pub offset: u32,
}

/// The pixel format of a texture, `u32` in the header and `u8` per mipmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    P8 = 0,
    /// `.paa` type `0x8080`, gray with alpha.
    Ai88 = 1,
    Rgb565 = 2,
    /// `.paa` type `0x1555`.
    Argb1555 = 3,
    /// `.paa` type `0x4444`.
    Argb4444 = 4,
    /// `.paa` type `0x8888`.
    Argb8888 = 5,
    /// `.paa` types `0xff01` to `0xff05`.
    Dxt1 = 6,
    Dxt2 = 7,
    Dxt3 = 8,
    Dxt4 = 9,
    Dxt5 = 10,
}

/// What a texture is for, from its file name, see [`TextureType::from_path`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureType {
    /// `0`: `_co`, `_ca`, satellite tiles (`s_###_###_lco`) and every other name.
    Diffuse,
    /// `1`: `_lco`, `_sky`.
    DiffuseLinear,
    /// `2`: `_dt`, `_cdt`, `_detail`, `_mco`.
    Detail,
    /// `3`: `_no`, `_non`, `_nopx`, `_noex`, `_nohq`, `_novhq`, `_nofhq`, `_nof`, `_nofex`, `_ns`,
    /// `_nsex`, `_nshq`, `_normalmap`.
    Normal,
    /// `7`: `_mc`.
    Macro,
    /// `8`: `_as`.
    AmbientShadow,
    /// `9`: `_sm`, `_smdi`.
    Specular,
    /// `11`: `_dtsmdi`.
    DetailSpecular,
    /// `12`: `_mask`.
    Mask,
    /// `13`: `_ti_ca`.
    Thermal,
    /// Values Binarize doesn't give file names (`4`, `5`, `6`, `10`) and unknown ones.
    Other(u32),
}

/// Suffixes Binarize knows, matched against the file name from its last `_`.
const SUFFIXES: [(&str, TextureType); 27] = [
    ("_co.", TextureType::Diffuse),
    ("_ca.", TextureType::Diffuse),
    ("_detail.", TextureType::Detail),
    ("_cdt.", TextureType::Detail),
    ("_dt.", TextureType::Detail),
    ("_mco.", TextureType::Detail),
    ("_no.", TextureType::Normal),
    ("_non.", TextureType::Normal),
    ("_nopx.", TextureType::Normal),
    ("_noex.", TextureType::Normal),
    ("_nohq.", TextureType::Normal),
    ("_novhq.", TextureType::Normal),
    ("_nofhq.", TextureType::Normal),
    ("_nof.", TextureType::Normal),
    ("_nofex.", TextureType::Normal),
    ("_ns.", TextureType::Normal),
    ("_nsex.", TextureType::Normal),
    ("_nshq.", TextureType::Normal),
    ("_normalmap.", TextureType::Normal),
    ("_mask.", TextureType::Mask),
    ("_mc.", TextureType::Macro),
    ("_as.", TextureType::AmbientShadow),
    ("_sm.", TextureType::Specular),
    ("_smdi.", TextureType::Specular),
    ("_dtsmdi.", TextureType::DetailSpecular),
    ("_sky.", TextureType::DiffuseLinear),
    ("_lco.", TextureType::DiffuseLinear),
];

impl PixelFormat {
    #[must_use]
    pub const fn from_u32(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::P8,
            1 => Self::Ai88,
            2 => Self::Rgb565,
            3 => Self::Argb1555,
            4 => Self::Argb4444,
            5 => Self::Argb8888,
            6 => Self::Dxt1,
            7 => Self::Dxt2,
            8 => Self::Dxt3,
            9 => Self::Dxt4,
            10 => Self::Dxt5,
            _ => return None,
        })
    }

    /// The format of a `.paa` type. Binarize takes all of them but `RGBA8888`, which it didn't
    /// read in the probes.
    #[must_use]
    pub const fn from_paa_type(paa_type: PaaType) -> Option<Self> {
        Some(match paa_type {
            PaaType::DXT1 => Self::Dxt1,
            PaaType::DXT2 => Self::Dxt2,
            PaaType::DXT3 => Self::Dxt3,
            PaaType::DXT4 => Self::Dxt4,
            PaaType::DXT5 => Self::Dxt5,
            PaaType::RGBA4444 => Self::Argb4444,
            PaaType::ARGB1555 => Self::Argb1555,
            PaaType::RGBA8888 => Self::Argb8888,
            PaaType::GRAYwAlpha => Self::Ai88,
            PaaType::UNKNOWN => return None,
        })
    }
}

impl TextureType {
    #[must_use]
    pub const fn from_u32(value: u32) -> Self {
        match value {
            0 => Self::Diffuse,
            1 => Self::DiffuseLinear,
            2 => Self::Detail,
            3 => Self::Normal,
            7 => Self::Macro,
            8 => Self::AmbientShadow,
            9 => Self::Specular,
            11 => Self::DetailSpecular,
            12 => Self::Mask,
            13 => Self::Thermal,
            other => Self::Other(other),
        }
    }

    #[must_use]
    pub const fn to_u32(self) -> u32 {
        match self {
            Self::Diffuse => 0,
            Self::DiffuseLinear => 1,
            Self::Detail => 2,
            Self::Normal => 3,
            Self::Macro => 7,
            Self::AmbientShadow => 8,
            Self::Specular => 9,
            Self::DetailSpecular => 11,
            Self::Mask => 12,
            Self::Thermal => 13,
            Self::Other(other) => other,
        }
    }

    /// The type Binarize gives a texture: the file name (without directories, case
    /// insensitive) from its last `_` up to and including a `.` has to be one of the known
    /// suffixes, e.g. `_nohq.` in `a_nohq.paa` or `a_nohq.x.paa`. `_ca.` after `_ti` is
    /// [`TextureType::Thermal`]. `_lco.` is [`TextureType::Diffuse`] for satellite tiles: a
    /// name starting with `s_` whose last `_` is its 10th character, like `s_004_024_lco.paa`.
    #[must_use]
    pub fn from_path(path: &str) -> Self {
        let name = path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(path)
            .to_ascii_lowercase();
        let Some(pos) = name.rfind('_') else {
            return Self::Diffuse;
        };
        let suffix = &name[pos..];
        let Some(&(found, texture_type)) = SUFFIXES.iter().find(|(s, _)| suffix.starts_with(s))
        else {
            return Self::Diffuse;
        };
        match found {
            "_ca." if name[..pos].ends_with("_ti") => Self::Thermal,
            "_lco." if name.starts_with("s_") && pos == 9 => Self::Diffuse,
            _ => texture_type,
        }
    }
}

impl TexHeader {
    /// The header Binarize writes for a texture: `path` is its path relative to the PBO (stored
    /// lower case with `\`), `paa` the file. Reads the type, the `AVGC`, `MAXC` and `FLAG` tags
    /// and the mipmap headers, not the pixels.
    pub fn from_paa(path: &str, paa: &[u8]) -> Result<Self, TexHeadersError> {
        let mut input = Reader { bytes: paa, pos: 0 };
        let paa_type = input.u16()?;
        let pixel_format = PaaType::try_from(paa_type)
            .ok()
            .and_then(PixelFormat::from_paa_type)
            .ok_or(TexHeadersError::UnknownPaaType(paa_type))?;

        let mut average = None;
        let mut max = None;
        let mut flags = None;
        while input.peek(4).is_ok_and(|tag| tag == b"GGAT") {
            input.take(4)?;
            let name: [u8; 4] = input.take(4)?.try_into().unwrap_or_default();
            let size = input.u32()? as usize;
            let data = input.take(size)?;
            let slot = match &name {
                b"CGVA" => &mut average,
                b"CXAM" => &mut max,
                b"GALF" => &mut flags,
                _ => continue,
            };
            let value: [u8; 4] = data
                .try_into()
                .map_err(|_| TexHeadersError::InvalidTag(lossy(&name), size))?;
            *slot = Some(u32::from_le_bytes(value));
        }

        let palette = input.u16()?;
        input.take(usize::from(palette) * 3)?;

        let mut mipmaps = Vec::new();
        loop {
            let offset = u32::try_from(input.pos).map_err(|_| TexHeadersError::TooLarge)?;
            let width = input.u16()?;
            let height = input.u16()?;
            if width == 0 && height == 0 {
                break;
            }
            let size = input.take(3)?;
            let size = u32::from_le_bytes([size[0], size[1], size[2], 0]) as usize;
            input.take(size)?;
            mipmaps.push(TexHeaderMipmap {
                width: width & 0x7fff,
                height,
                reserved: 0,
                pixel_format,
                unknown: 3,
                offset,
            });
        }

        let average = average.unwrap_or(DEFAULT_AVERAGE_COLOR).to_le_bytes();
        let [b, g, r, a] = average.map(|c| f32::from(c) * (1.0 / 255.0));
        let flags = flags.unwrap_or(0);
        let is_alpha = flags & 1 != 0;
        let path = path.replace('/', "\\").to_ascii_lowercase();
        Ok(Self {
            color_palette_count: 1,
            palette_offset: 0,
            average_color: [r, g, b, a],
            average_color_packed: 0,
            max_color: max.unwrap_or(DEFAULT_MAX_COLOR),
            clamp_flags: 0,
            transparent_color: 0xffff_ffff,
            has_max_color: max.is_some(),
            is_alpha,
            is_transparent: flags & 2 != 0,
            is_alpha_non_opaque: is_alpha && average[3] < 128,
            pixel_format,
            little_endian: true,
            is_paa: !path
                .rsplit_once('.')
                .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("pac")),
            texture_type: TextureType::from_path(&path),
            path,
            mipmaps,
            file_size: u32::try_from(paa.len()).map_err(|_| TexHeadersError::TooLarge)?,
        })
    }

    fn parse(input: &mut Reader) -> Result<Self, TexHeadersError> {
        let color_palette_count = input.u32()?;
        let palette_offset = input.u32()?;
        let average_color = [input.f32()?, input.f32()?, input.f32()?, input.f32()?];
        let average_color_packed = input.u32()?;
        let max_color = input.u32()?;
        let clamp_flags = input.u32()?;
        let transparent_color = input.u32()?;
        let [has_max_color, is_alpha, is_transparent, is_alpha_non_opaque] =
            [input.bool()?, input.bool()?, input.bool()?, input.bool()?];
        let mipmap_count = input.u32()?;
        let pixel_format = input.pixel_format()?;
        let little_endian = input.bool()?;
        let is_paa = input.bool()?;
        let path = input.cstring()?;
        let texture_type = TextureType::from_u32(input.u32()?);
        let mipmap_count_2 = input.u32()?;
        if mipmap_count != mipmap_count_2 {
            return Err(TexHeadersError::MipmapCountMismatch(
                mipmap_count,
                mipmap_count_2,
            ));
        }
        // The count is untrusted, don't allocate more than the data can hold.
        let mut mipmaps = Vec::with_capacity((mipmap_count as usize).min(input.remaining() / 12));
        for _ in 0..mipmap_count {
            mipmaps.push(TexHeaderMipmap {
                width: input.u16()?,
                height: input.u16()?,
                reserved: input.u16()?,
                pixel_format: {
                    let format = input.u8()?;
                    PixelFormat::from_u32(u32::from(format))
                        .ok_or_else(|| TexHeadersError::UnknownPixelFormat(u32::from(format)))?
                },
                unknown: input.u8()?,
                offset: input.u32()?,
            });
        }
        Ok(Self {
            color_palette_count,
            palette_offset,
            average_color,
            average_color_packed,
            max_color,
            clamp_flags,
            transparent_color,
            has_max_color,
            is_alpha,
            is_transparent,
            is_alpha_non_opaque,
            pixel_format,
            little_endian,
            is_paa,
            path,
            texture_type,
            mipmaps,
            file_size: input.u32()?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<(), TexHeadersError> {
        if self.path.contains('\0') {
            return Err(TexHeadersError::InvalidPath(self.path.clone()));
        }
        let count = u32::try_from(self.mipmaps.len()).map_err(|_| TexHeadersError::TooLarge)?;
        out.extend(self.color_palette_count.to_le_bytes());
        out.extend(self.palette_offset.to_le_bytes());
        for c in self.average_color {
            out.extend(c.to_le_bytes());
        }
        out.extend(self.average_color_packed.to_le_bytes());
        out.extend(self.max_color.to_le_bytes());
        out.extend(self.clamp_flags.to_le_bytes());
        out.extend(self.transparent_color.to_le_bytes());
        out.extend([
            u8::from(self.has_max_color),
            u8::from(self.is_alpha),
            u8::from(self.is_transparent),
            u8::from(self.is_alpha_non_opaque),
        ]);
        out.extend(count.to_le_bytes());
        out.extend((self.pixel_format as u32).to_le_bytes());
        out.extend([u8::from(self.little_endian), u8::from(self.is_paa)]);
        out.extend(self.path.as_bytes());
        out.push(0);
        out.extend(self.texture_type.to_u32().to_le_bytes());
        out.extend(count.to_le_bytes());
        for m in &self.mipmaps {
            out.extend(m.width.to_le_bytes());
            out.extend(m.height.to_le_bytes());
            out.extend(m.reserved.to_le_bytes());
            out.extend([m.pixel_format as u8, m.unknown]);
            out.extend(m.offset.to_le_bytes());
        }
        out.extend(self.file_size.to_le_bytes());
        Ok(())
    }
}

impl TexHeaders {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        Ok(Self::parse(&bytes)?)
    }

    fn parse(bytes: &[u8]) -> Result<Self, TexHeadersError> {
        let mut input = Reader { bytes, pos: 0 };
        let signature = input.take(4)?;
        if signature != TEXHEADERS_SIGNATURE {
            return Err(TexHeadersError::UnknownSignature(lossy(signature)));
        }
        let version = input.u32()?;
        if version != TEXHEADERS_VERSION {
            return Err(TexHeadersError::UnsupportedVersion(version));
        }
        let count = input.u32()?;
        // A texture takes at least 79 bytes.
        let mut textures = Vec::with_capacity((count as usize).min(input.remaining() / 79));
        for _ in 0..count {
            textures.push(TexHeader::parse(&mut input)?);
        }
        if input.remaining() != 0 {
            return Err(TexHeadersError::TrailingData(input.remaining()));
        }
        Ok(Self { version, textures })
    }

    /// Writes the headers at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let count = u32::try_from(self.textures.len()).map_err(|_| TexHeadersError::TooLarge)?;
        let mut bytes = Vec::new();
        bytes.extend(TEXHEADERS_SIGNATURE);
        bytes.extend(self.version.to_le_bytes());
        bytes.extend(count.to_le_bytes());
        for texture in &self.textures {
            texture.write(&mut bytes)?;
        }
        out.write_all(&bytes)?;
        Ok(())
    }

    /// The headers of a PBO's textures in Binarize's order, see
    /// [`TexHeaders::sort_like_binarize`].
    #[must_use]
    pub fn from_textures(textures: Vec<TexHeader>) -> Self {
        let mut headers = Self {
            version: TEXHEADERS_VERSION,
            textures,
        };
        headers.sort_like_binarize();
        headers
    }

    /// Puts the textures in the order Binarize writes them. It walks the folder like Windows
    /// lists it (each directory sorted by name, case insensitive, subdirectories at their place
    /// among the files) and inserts the paths into a hash table with 15 buckets, that grows to
    /// `2 * buckets + 1` before an insert would put more than 16 paths per bucket. The hash of a
    /// path is `hash * 33 + byte` over its bytes, from `0` in `u32`. The file lists the buckets
    /// in order, each in insertion order (growing keeps that order).
    pub fn sort_like_binarize(&mut self) {
        self.textures.sort_by_cached_key(|t| {
            t.path
                .split('\\')
                .map(str::to_ascii_uppercase)
                .collect::<Vec<_>>()
        });
        let hash = |t: &TexHeader| {
            t.path
                .bytes()
                .fold(0u32, |h, b| h.wrapping_mul(33).wrapping_add(u32::from(b)))
        };
        let mut buckets: Vec<Vec<TexHeader>> = vec![Vec::new(); 15];
        for (count, texture) in std::mem::take(&mut self.textures).into_iter().enumerate() {
            if count + 1 > 16 * buckets.len() {
                let mut grown = vec![Vec::new(); buckets.len() * 2 + 1];
                for t in buckets.into_iter().flatten() {
                    let len = grown.len();
                    grown[hash(&t) as usize % len].push(t);
                }
                buckets = grown;
            }
            let len = buckets.len();
            buckets[hash(&texture) as usize % len].push(texture);
        }
        self.textures = buckets.into_iter().flatten().collect();
    }
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
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

    fn peek(&self, len: usize) -> Result<&'a [u8], TexHeadersError> {
        self.pos
            .checked_add(len)
            .and_then(|end| self.bytes.get(self.pos..end))
            .ok_or(TexHeadersError::Truncated)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], TexHeadersError> {
        let bytes = self.peek(len)?;
        self.pos += len;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, TexHeadersError> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> Result<bool, TexHeadersError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(TexHeadersError::InvalidBool(other)),
        }
    }

    fn u16(&mut self) -> Result<u16, TexHeadersError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().unwrap_or_default(),
        ))
    }

    fn u32(&mut self) -> Result<u32, TexHeadersError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }

    fn f32(&mut self) -> Result<f32, TexHeadersError> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn pixel_format(&mut self) -> Result<PixelFormat, TexHeadersError> {
        let value = self.u32()?;
        PixelFormat::from_u32(value).ok_or(TexHeadersError::UnknownPixelFormat(value))
    }

    fn cstring(&mut self) -> Result<String, TexHeadersError> {
        let rest = &self.bytes[self.pos..];
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(TexHeadersError::Truncated)?;
        let string = String::from_utf8(rest[..len].to_vec())
            .map_err(|e| TexHeadersError::InvalidString(e.into_bytes()))?;
        self.pos += len + 1;
        Ok(string)
    }
}
