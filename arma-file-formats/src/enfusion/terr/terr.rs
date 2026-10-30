use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use binrw::{BinRead, BinWrite, binrw};

use crate::{
    core::{
        iff::{ChunkHeader, FormHeader},
        read::ReadExtTrait,
    },
    errors::{AffError, TerrError},
};

const TERR_FORM_TYPE: &[u8; 4] = b"TERR";
/// First version that stores [`Terr::surface_bits`].
pub const TERR_SURFACE_BITS_VERSION: u32 = 9;
/// Surface weight bits of terrains that do not store them.
pub const TERR_DEFAULT_SURFACE_BITS: u16 = 8;

/// Enfusion terrain descriptor (`.terr`).
#[derive(Debug, Clone, PartialEq)]
pub struct Terr {
    /// Contents of the `VERS` chunk.
    pub version: u32,
    pub head: TerrHead,
    pub texture_sizes: TerrTextureSizes,
    /// Bits per surface weight in the tile layers: `8` allows 5 surfaces per block, `5` allows
    /// 7. Only stored since [`TERR_SURFACE_BITS_VERSION`], older terrains use
    /// [`TERR_DEFAULT_SURFACE_BITS`].
    pub surface_bits: u16,
    /// Resource paths of the surface materials, prefixed with their GUID, e.g.
    /// `{E755137CEA663544}system/terrain/default.emat`. Tile layers refer to them by index.
    pub materials: Vec<String>,
    /// `GRAS` chunk.
    pub grass: Option<TerrGrass>,
    /// Chunks this reader does not know, in file order. They are written after the known chunks.
    pub unknown_chunks: Vec<TerrChunk>,
}

/// Contents of the `HEAD` chunk: the size and layout of the height grid.
///
/// The terrain is split into tiles, each tile into `blocks_per_tile` x `blocks_per_tile` blocks
/// and each block into `block_vertices - 1` x `block_vertices - 1` grid cells.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrHead {
    /// Vertices along the x axis, the width in grid cells plus one.
    pub vertices_x: u32,
    /// Vertices along the z axis, the height in grid cells plus one.
    pub vertices_z: u32,
    /// Vertices along one side of a block, `33`.
    pub block_vertices: u32,
    /// Blocks along one side of a tile.
    pub blocks_per_tile: u32,
    /// Distance between neighboring vertices in meters.
    pub grid_cell_size: f32,
    /// Height difference in meters between two consecutive values of the 16 bit height map.
    pub height_scale: f32,
    /// Height in meters of the height map value `0`. Workbench sets it from the "zero height"
    /// percentage of the 16 bit range that lies below 0 m: `-floor(65535 * percent / 100) *
    /// height_scale`, `-204.78125` for the default of 10 %.
    pub height_offset: f32,
    /// Has always been equal to [`Terr::version`].
    pub unknown_28: u32,
}

impl TerrHead {
    pub const SIZE: u32 = 32;

    /// Grid cells along one side of a tile.
    #[must_use]
    pub const fn tile_cells(&self) -> u32 {
        self.block_vertices.saturating_sub(1) * self.blocks_per_tile
    }

    /// Tiles along the x and z axis.
    #[must_use]
    pub const fn tiles(&self) -> [u32; 2] {
        let tile_cells = self.tile_cells();
        if tile_cells == 0 {
            return [0, 0];
        }
        [
            self.vertices_x.saturating_sub(1).div_ceil(tile_cells),
            self.vertices_z.saturating_sub(1).div_ceil(tile_cells),
        ]
    }

    /// Number of `.ttile` files.
    #[must_use]
    pub const fn tile_count(&self) -> u32 {
        let [x, z] = self.tiles();
        x * z
    }

    /// Height in meters of a height map value.
    #[must_use]
    pub fn height(&self, value: u16) -> f32 {
        f32::from(value).mul_add(self.height_scale, self.height_offset)
    }

    /// Extent of the terrain along the x and z axis in meters.
    #[must_use]
    pub fn size_meters(&self) -> [f32; 2] {
        [
            self.vertices_x.saturating_sub(1) as f32 * self.grid_cell_size,
            self.vertices_z.saturating_sub(1) as f32 * self.grid_cell_size,
        ]
    }
}

/// Contents of the `TEXS` chunk: the resolution of the textures of every tile, `0` if the tiles
/// have none, as in a newly created terrain.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrTextureSizes {
    /// `<name>_<index>_supertexture.edds`, the color texture shown from a distance.
    pub super_texture: u16,
    /// `<name>_<index>_normal.edds`.
    pub normal: u16,
    /// `<name>_<index>_layer.edds`, the surface weights.
    pub layer: u16,
}

impl TerrTextureSizes {
    pub const SIZE: u32 = 6;
}

/// Contents of the `GRAS` chunk.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrGrass {
    /// Wanted edge length of a grass cluster in meters. The engine rounds it so that a block
    /// holds [`TerrGrass::clusters_per_block`] clusters along each side.
    pub cluster_size: f32,
    /// Grass clusters along one side of a block, `0` in a newly created terrain.
    pub clusters_per_block: u16,
}

impl TerrGrass {
    pub const SIZE: u32 = 6;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrChunk {
    pub tag: [u8; 4],
    pub data: Vec<u8>,
}

impl Terr {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let mut chunks = read_chunks(input)?;
        let mut take = |tag: &'static [u8; 4]| {
            chunks
                .iter()
                .position(|c| &c.tag == tag)
                .map(|i| chunks.remove(i))
        };

        let vers = take(b"VERS").ok_or(TerrError::ChunkMissing("VERS"))?;
        let version = u32::from_le_bytes(
            vers.data
                .as_slice()
                .try_into()
                .map_err(|_| TerrError::InvalidChunkSize("VERS"))?,
        );

        let head = read_fixed(take(b"HEAD"), "HEAD", TerrHead::SIZE)?;
        let texture_sizes = read_fixed(take(b"TEXS"), "TEXS", TerrTextureSizes::SIZE)?;
        let mats = take(b"MATS").ok_or(TerrError::ChunkMissing("MATS"))?;
        let (surface_bits, materials) = read_materials(&mats.data, version)?;
        let grass = take(b"GRAS")
            .map(|c| read_fixed(Some(c), "GRAS", TerrGrass::SIZE))
            .transpose()?;

        Ok(Self {
            version,
            head,
            texture_sizes,
            surface_bits,
            materials,
            grass,
            unknown_chunks: chunks,
        })
    }

    /// Writes the terr at the current position of `out`. [`Terr::surface_bits`] is only written
    /// if [`Terr::version`] stores it.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let mut body = Cursor::new(Vec::new());
        body.write_all(TERR_FORM_TYPE)?;

        write_chunk(&mut body, *b"VERS", &self.version.to_le_bytes())?;
        write_chunk(&mut body, *b"HEAD", &to_bytes(&self.head)?)?;
        write_chunk(&mut body, *b"TEXS", &to_bytes(&self.texture_sizes)?)?;

        let mut mats = Vec::new();
        if self.version >= TERR_SURFACE_BITS_VERSION {
            mats.extend_from_slice(&self.surface_bits.to_le_bytes());
        }
        for material in &self.materials {
            let len = u32::try_from(material.len() + 1).map_err(|_| TerrError::TooLarge)?;
            mats.extend_from_slice(&len.to_le_bytes());
            mats.extend_from_slice(material.as_bytes());
            mats.push(0);
        }
        write_chunk(&mut body, *b"MATS", &mats)?;

        if let Some(grass) = &self.grass {
            write_chunk(&mut body, *b"GRAS", &to_bytes(grass)?)?;
        }
        for chunk in &self.unknown_chunks {
            write_chunk(&mut body, chunk.tag, &chunk.data)?;
        }

        let body = body.into_inner();
        let size = u32::try_from(body.len()).map_err(|_| TerrError::TooLarge)?;
        out.write_all(b"FORM")?;
        out.write_all(&size.to_be_bytes())?;
        out.write_all(&body)?;
        Ok(())
    }
}

fn read_chunks<I: Seek + Read>(input: &mut I) -> Result<Vec<TerrChunk>, AffError> {
    let start = input.stream_position()?;
    let form = FormHeader::read_form(input)?;
    if &form.form_type != TERR_FORM_TYPE {
        return Err(TerrError::UnknownFormType(form.form_type_lossy()).into());
    }
    let form_end = form.end(start);

    let mut chunks = Vec::new();
    while input.stream_position()? < form_end {
        let header = ChunkHeader::read(input)?;
        let data = input.read_bytes(header.size as usize)?;
        chunks.push(TerrChunk {
            tag: header.tag,
            data,
        });
    }
    input.seek(SeekFrom::Start(form_end))?;

    Ok(chunks)
}

fn read_fixed<T>(chunk: Option<TerrChunk>, tag: &'static str, size: u32) -> Result<T, AffError>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    let chunk = chunk.ok_or(TerrError::ChunkMissing(tag))?;
    if chunk.data.len() != size as usize {
        return Err(TerrError::InvalidChunkSize(tag).into());
    }
    Ok(T::read_le(&mut Cursor::new(&chunk.data))?)
}

fn read_materials(mut data: &[u8], version: u32) -> Result<(u16, Vec<String>), TerrError> {
    let mut surface_bits = TERR_DEFAULT_SURFACE_BITS;
    if version >= TERR_SURFACE_BITS_VERSION {
        let (bits, rest) = data
            .split_first_chunk::<2>()
            .ok_or(TerrError::InvalidChunkSize("MATS"))?;
        surface_bits = u16::from_le_bytes(*bits);
        data = rest;
    }

    let mut materials = Vec::new();
    while !data.is_empty() {
        let index = materials.len();
        let (len, rest) = data
            .split_first_chunk::<4>()
            .ok_or(TerrError::InvalidMaterial(index))?;
        let len = u32::from_le_bytes(*len) as usize;
        let (path, rest) = rest
            .split_at_checked(len)
            .ok_or(TerrError::InvalidMaterial(index))?;
        let path = path
            .strip_suffix(&[0])
            .ok_or(TerrError::InvalidMaterial(index))?;
        materials.push(String::from_utf8_lossy(path).into_owned());
        data = rest;
    }

    Ok((surface_bits, materials))
}

fn write_chunk<W: Write + Seek>(out: &mut W, tag: [u8; 4], data: &[u8]) -> Result<(), AffError> {
    let size = u32::try_from(data.len()).map_err(|_| TerrError::TooLarge)?;
    ChunkHeader { tag, size }.write(out)?;
    out.write_all(data)?;
    Ok(())
}

fn to_bytes<T>(value: &T) -> Result<Vec<u8>, AffError>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    let mut out = Cursor::new(Vec::new());
    value.write_le(&mut out)?;
    Ok(out.into_inner())
}
