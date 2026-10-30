use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use binrw::{BinRead, BinWrite, binrw, helpers::until_eof, io::NoSeek};

use crate::{
    core::iff::FormHeader,
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

/// A chunk of the `FORM`, with a big endian tag and size.
#[binrw]
#[brw(big)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrChunk {
    pub tag: [u8; 4],
    #[br(temp)]
    #[bw(try_calc = u32::try_from(data.len()))]
    size: u32,
    #[br(count = size)]
    pub data: Vec<u8>,
}

/// Contents of the `MATS` chunk.
#[binrw]
#[brw(little, import(version: u32))]
struct TerrMaterials {
    #[br(if(version >= TERR_SURFACE_BITS_VERSION, TERR_DEFAULT_SURFACE_BITS))]
    #[bw(if(version >= TERR_SURFACE_BITS_VERSION))]
    surface_bits: u16,
    #[br(parse_with = until_eof)]
    paths: Vec<MaterialPath>,
}

/// A `u32` length and a NUL terminated string, the length includes the NUL.
#[binrw]
#[brw(little)]
struct MaterialPath {
    #[br(temp)]
    #[bw(try_calc = u32::try_from(path.len() + 1))]
    len: u32,
    #[br(count = len, try_map = strip_nul)]
    #[bw(map = |path: &String| [path.as_bytes(), &[0]].concat())]
    path: String,
}

impl Terr {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let mut chunks = read_chunks(input)?;
        let mut take = |tag: &'static str| {
            chunks
                .iter()
                .position(|c| c.tag == tag.as_bytes())
                .map(|i| chunks.remove(i))
        };
        let mut required = |tag| take(tag).ok_or(TerrError::ChunkMissing(tag));

        let version = parse(&required("VERS")?, "VERS", ())?;
        let head = parse(&required("HEAD")?, "HEAD", ())?;
        let texture_sizes = parse(&required("TEXS")?, "TEXS", ())?;
        let mats: TerrMaterials = parse(&required("MATS")?, "MATS", (version,))?;
        let grass = take("GRAS").map(|c| parse(&c, "GRAS", ())).transpose()?;

        Ok(Self {
            version,
            head,
            texture_sizes,
            surface_bits: mats.surface_bits,
            materials: mats.paths.into_iter().map(|m| m.path).collect(),
            grass,
            unknown_chunks: chunks,
        })
    }

    /// Writes the terr at the current position of `out`. [`Terr::surface_bits`] is only written
    /// if [`Terr::version`] stores it.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let mats = TerrMaterials {
            surface_bits: self.surface_bits,
            paths: self
                .materials
                .iter()
                .map(|path| MaterialPath { path: path.clone() })
                .collect(),
        };
        let mut chunks = vec![
            chunk(*b"VERS", &self.version, ())?,
            chunk(*b"HEAD", &self.head, ())?,
            chunk(*b"TEXS", &self.texture_sizes, ())?,
            chunk(*b"MATS", &mats, (self.version,))?,
        ];
        if let Some(grass) = &self.grass {
            chunks.push(chunk(*b"GRAS", grass, ())?);
        }

        let mut body = Cursor::new(Vec::new());
        for chunk in chunks.iter().chain(&self.unknown_chunks) {
            chunk.write(&mut body)?;
        }
        let body = body.into_inner();

        FormHeader {
            size: u32::try_from(body.len() + 4).map_err(|_| TerrError::TooLarge)?,
            form_type: *TERR_FORM_TYPE,
        }
        .write(&mut NoSeek::new(&mut *out))?;
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
        chunks.push(TerrChunk::read(input)?);
    }
    input.seek(SeekFrom::Start(form_end))?;

    Ok(chunks)
}

/// Parses the contents of a chunk, which have to be used up exactly.
fn parse<T, A>(chunk: &TerrChunk, tag: &'static str, args: A) -> Result<T, TerrError>
where
    T: for<'a> BinRead<Args<'a> = A>,
{
    let mut cursor = Cursor::new(&chunk.data);
    T::read_le_args(&mut cursor, args)
        .ok()
        .filter(|_| cursor.position() == chunk.data.len() as u64)
        .ok_or(TerrError::InvalidChunk(tag))
}

fn chunk<T, A>(tag: [u8; 4], value: &T, args: A) -> Result<TerrChunk, AffError>
where
    T: for<'a> BinWrite<Args<'a> = A>,
{
    let mut data = Cursor::new(Vec::new());
    value.write_le_args(&mut data, args)?;
    Ok(TerrChunk {
        tag,
        data: data.into_inner(),
    })
}

fn strip_nul(mut bytes: Vec<u8>) -> Result<String, &'static str> {
    if bytes.pop() != Some(0) {
        return Err("material path is not NUL terminated");
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
