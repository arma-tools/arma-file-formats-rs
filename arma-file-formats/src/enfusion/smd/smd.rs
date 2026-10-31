use std::{
    io::{Cursor, Read, Seek, SeekFrom, Write},
    num::NonZeroU16,
};

use binrw::{BinRead, BinWrite, binrw, io::NoSeek};

use crate::{
    core::iff::FormHeader,
    errors::{AffError, SmdError},
};

const SMD_FORM_TYPE: &[u8; 4] = b"SMDF";
/// The only known version.
pub const SMD_VERSION: u32 = 2;

/// Enfusion shore map data (`.smd`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq)]
pub struct Smd {
    /// Contents of the `VERS` chunk, [`SMD_VERSION`].
    pub version: u32,
    pub head: SmdHead,
    pub tile_map: SmdTileMap,
    /// Chunks this reader does not know, in file order. They are written after the known chunks.
    pub unknown_chunks: Vec<SmdChunk>,
}

/// Contents of the `HEAD` chunk. Coordinates are world `x` and `z` in meters.
///
/// The extent is the bounding box of the coast grown by `DistanceFieldMaxValue` of the shore map
/// config (300 m in the game's worlds), extended to a whole number of tiles: `max - min = tiles *
/// tile_size`, and `tile_size = tile_pixels * meters_per_pixel`.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmdHead {
    /// Lowest `x` and `z` of the tile map.
    pub min: [f32; 2],
    /// Highest `x` and `z` of the tile map.
    pub max: [f32; 2],
    /// Tiles with shore textures, the highest index in [`SmdTileMap::tiles`].
    pub tile_count: u32,
    /// Edge length of an atlas slot in pixels, [`SmdHead::tile_pixels`] plus a 1 pixel border on
    /// each side.
    pub slot_pixels: u32,
    /// Pixels along one side of a tile, without the border.
    pub tile_pixels: u32,
    /// Edge length of a tile in meters.
    pub tile_size: f32,
    pub meters_per_pixel: f32,
    /// Slots in the atlases along `x` and `y`, at least [`SmdHead::tile_count`]. The atlases are
    /// `atlas_slots * slot_pixels` pixels.
    pub atlas_slots: [u32; 2],
}

impl SmdHead {
    /// Size of the atlas textures in pixels.
    #[must_use]
    pub const fn atlas_size(&self) -> [u32; 2] {
        [
            self.atlas_slots[0] * self.slot_pixels,
            self.atlas_slots[1] * self.slot_pixels,
        ]
    }

    /// Pixel position of the top left corner of a tile's slot in the atlases, including the
    /// border. The slots are filled row by row, `index` 1 is the first one.
    #[must_use]
    pub const fn atlas_slot(&self, index: NonZeroU16) -> [u32; 2] {
        let slot = index.get() as u32 - 1;
        let columns = if self.atlas_slots[0] == 0 {
            1
        } else {
            self.atlas_slots[0]
        };
        [
            slot % columns * self.slot_pixels,
            slot / columns * self.slot_pixels,
        ]
    }
}

/// Contents of the `TMAP` chunk: the atlas slot of every tile.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SmdTileMap {
    /// Tiles along `x`.
    pub width: u32,
    /// Tiles along `z`.
    pub height: u32,
    /// Row by row along `x`, the first row at [`SmdHead::min`] `z`. `0` for no shore, otherwise
    /// the 1 based atlas slot, numbered in this order. Has `width * height` entries.
    #[br(count = u64::from(width) * u64::from(height))]
    pub tiles: Vec<u16>,
}

impl SmdTileMap {
    /// The atlas slot of the tile in column `x` and row `z`, `None` without shore or outside the
    /// map.
    #[must_use]
    pub fn tile(&self, x: u32, z: u32) -> Option<NonZeroU16> {
        if x >= self.width || z >= self.height {
            return None;
        }
        let index = z as usize * self.width as usize + x as usize;
        self.tiles.get(index).copied().and_then(NonZeroU16::new)
    }
}

/// A chunk of the `FORM`, with a big endian tag and size.
#[binrw]
#[brw(big)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmdChunk {
    pub tag: [u8; 4],
    #[br(temp)]
    #[bw(try_calc = u32::try_from(data.len()))]
    size: u32,
    #[br(count = size)]
    pub data: Vec<u8>,
}

impl Smd {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let start = input.stream_position()?;
        let form = FormHeader::read_form(input)?;
        if &form.form_type != SMD_FORM_TYPE {
            return Err(SmdError::UnknownFormType(form.form_type_lossy()).into());
        }
        let form_end = form.end(start);
        let mut chunks = Vec::new();
        while input.stream_position()? < form_end {
            chunks.push(SmdChunk::read(input)?);
        }
        input.seek(SeekFrom::Start(form_end))?;

        let mut required = |tag: &'static str| {
            chunks
                .iter()
                .position(|c| c.tag == tag.as_bytes())
                .map(|i| chunks.remove(i))
                .ok_or(SmdError::ChunkMissing(tag))
        };
        let version = parse(&required("VERS")?, "VERS")?;
        if version != SMD_VERSION {
            return Err(SmdError::UnsupportedVersion(version).into());
        }
        let head = parse(&required("HEAD")?, "HEAD")?;
        let tile_map = parse(&required("TMAP")?, "TMAP")?;

        Ok(Self {
            version,
            head,
            tile_map,
            unknown_chunks: chunks,
        })
    }

    /// Writes the smd at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let map = &self.tile_map;
        if map.tiles.len() as u64 != u64::from(map.width) * u64::from(map.height) {
            return Err(SmdError::Unwritable("TMAP").into());
        }
        let chunks = [
            chunk(*b"VERS", &self.version)?,
            chunk(*b"HEAD", &self.head)?,
            chunk(*b"TMAP", map)?,
        ];

        let mut body = Cursor::new(Vec::new());
        for chunk in chunks.iter().chain(&self.unknown_chunks) {
            chunk.write(&mut body)?;
        }
        let body = body.into_inner();
        FormHeader {
            size: u32::try_from(body.len() + 4).map_err(|_| SmdError::TooLarge)?,
            form_type: *SMD_FORM_TYPE,
        }
        .write(&mut NoSeek::new(&mut *out))?;
        out.write_all(&body)?;
        Ok(())
    }

    /// The atlas slot of the tile at a world position (`x`, `z`), `None` without shore or
    /// outside the map.
    #[must_use]
    pub fn tile_at(&self, [x, z]: [f32; 2]) -> Option<NonZeroU16> {
        let cell = |v: f32, min: f32| {
            let cell = ((v - min) / self.head.tile_size).floor();
            // Also rejects NaN.
            (cell >= 0.0 && cell < u32::MAX as f32).then_some(cell as u32)
        };
        self.tile_map
            .tile(cell(x, self.head.min[0])?, cell(z, self.head.min[1])?)
    }
}

/// Parses the contents of a chunk, which have to be used up exactly.
fn parse<T>(chunk: &SmdChunk, tag: &'static str) -> Result<T, SmdError>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    let mut cursor = Cursor::new(&chunk.data);
    T::read_le(&mut cursor)
        .ok()
        .filter(|_| cursor.position() == chunk.data.len() as u64)
        .ok_or(SmdError::InvalidChunk(tag))
}

/// Serializes `value` into a chunk.
fn chunk<T>(tag: [u8; 4], value: &T) -> Result<SmdChunk, AffError>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    let mut data = Cursor::new(Vec::new());
    value.write_le(&mut data)?;
    Ok(SmdChunk {
        tag,
        data: data.into_inner(),
    })
}
