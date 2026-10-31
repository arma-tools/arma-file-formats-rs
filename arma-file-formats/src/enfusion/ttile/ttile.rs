use std::io::{Cursor, Read, Seek, Write};

use binrw::{BinRead, BinResult, BinWrite, binrw, helpers::until_eof};

use super::materials::{TileBlockMaterials, read_block_materials, write_block_materials};
use crate::{
    enfusion::terr::{TerrChunk, chunk, parse, read_chunks, write_form},
    errors::{AffError, TerrError},
};

/// First version known to store `u32` block indices in `LRS2`, version 5 stores `u16`.
pub const TTILE_U32_BLOCK_INDEX_VERSION: u32 = 7;
/// First version with the compact `BERR` and `TMAT` layouts.
pub const TTILE_COMPACT_VERSION: u32 = 9;

/// Enfusion terrain tile (`.ttile`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq)]
pub struct Ttile {
    /// Contents of the `VERS` chunk, the same as [`Terr::version`](crate::enfusion::terr::Terr).
    pub version: u32,
    /// Height map values of the [`Ttile::vertices`] x [`Ttile::vertices`] vertices of the tile,
    /// row by row along x, starting at the lowest z. Neighboring tiles share their edge
    /// vertices. [`TerrHead::height`](crate::enfusion::terr::TerrHead::height) converts them to
    /// meters.
    pub heights: Vec<u16>,
    /// `BERR` chunk, one entry per block of the tile, row by row along x.
    pub lods: Vec<TileBlockLod>,
    /// `GCTD` chunk, absent if no block of the tile has grass.
    pub grass: Option<TileGrass>,
    /// `LRS2` chunk, the surface layers of each block of the tile, row by row along x.
    pub layers: Vec<TileBlockLayers>,
    /// `TMAT` chunk, the surface material map of each block of the tile, row by row along x.
    pub materials: Vec<TileBlockMaterials>,
    /// Chunks this reader does not know, in file order. They are written after the known chunks.
    pub unknown_chunks: Vec<TerrChunk>,
}

/// An entry of the `BERR` chunk: height range and level of detail errors of a block.
///
/// Heights are height map values stored as `f32`, unless noted otherwise. A newly created
/// terrain has not computed the errors yet: both errors are `0` and the
/// [`TileBlockLod::lod_errors`] are `1e6`.
///
/// In the game's terrains all errors are whole numbers of height map values, and they are larger
/// than the error of simplifying the block on its own, so what exactly they measure is unknown.
#[binrw]
#[brw(little, import(version: u32))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileBlockLod {
    /// Lowest height map value of the block.
    pub min_height: f32,
    /// Highest height map value of the block.
    pub max_height: f32,
    /// A whole number of height map values, at most `max_height - min_height`. In the game's
    /// terrains usually a half to the whole of the first of the [`TileBlockLod::lod_errors`],
    /// but up to about twice of it.
    pub unknown_8: f32,
    /// Largest level of detail error in meters.
    pub max_error: f32,
    /// Errors of the coarser levels of detail, as fractions of [`TileBlockLod::max_error`].
    /// They never decrease.
    pub lod_errors: [f32; 4],
    /// Only stored before [`TTILE_COMPACT_VERSION`]. The center of the block in meters in the
    /// game's version 5 terrains, `0` in version 7.
    #[br(if(version < TTILE_COMPACT_VERSION))]
    #[bw(if(version < TTILE_COMPACT_VERSION))]
    pub center_x: f32,
    /// Height of the center vertex of the block in meters, without the
    /// [`TerrHead::height_offset`](crate::enfusion::terr::TerrHead::height_offset). Workbench
    /// 1.8 and all game terrains of version 7 and 9 except Eden leave it out. Eden includes it
    /// in 94 % of its blocks, the others form patches. Version 5 terrains vary.
    pub center_y: f32,
    /// Only stored before [`TTILE_COMPACT_VERSION`], see [`TileBlockLod::center_x`].
    #[br(if(version < TTILE_COMPACT_VERSION))]
    #[bw(if(version < TTILE_COMPACT_VERSION))]
    pub center_z: f32,
}

/// Contents of the `GCTD` chunk: the grass clusters of the blocks that have grass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileGrass {
    /// Clusters per block, the square of
    /// [`TerrGrass::clusters_per_block`](crate::enfusion::terr::TerrGrass::clusters_per_block).
    pub block_clusters: u16,
    pub blocks: Vec<TileGrassBlock>,
}

/// The grass clusters of a block.
#[binrw]
#[brw(little, import(block_clusters: u16))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileGrassBlock {
    /// Block coordinates in the whole terrain, not the tile.
    pub x: u16,
    pub z: u16,
    /// Bitwise or of all [`TileGrassBlock::clusters`].
    pub layers: u8,
    /// One bit mask of grass layers per cluster, row by row along x.
    #[br(count = block_clusters)]
    pub clusters: Vec<u8>,
}

/// An entry of the `LRS2` chunk: the surface materials of a block.
///
/// They include all [`TileBlockMaterials::materials`] of the block. In the game's terrains about
/// one block in eight lists more, often materials of nearby blocks.
#[binrw]
#[brw(little, import(version: u32))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileBlockLayers {
    /// Index of the block in the whole terrain, row by row along x. Stored as `u16` before
    /// [`TTILE_U32_BLOCK_INDEX_VERSION`].
    #[br(parse_with = read_block_index, args(version))]
    #[bw(write_with = write_block_index, args(version))]
    pub block: u32,
    #[br(temp)]
    #[bw(try_calc = u16::try_from(materials.len()))]
    count: u16,
    /// Indices into [`Terr::materials`](crate::enfusion::terr::Terr::materials), sorted except
    /// in some version 5 blocks.
    #[br(count = count)]
    pub materials: Vec<u16>,
}

#[binrw]
#[brw(little, import(version: u32))]
struct Lods(
    #[br(parse_with = until_eof, args(version))]
    #[bw(args(version))]
    Vec<TileBlockLod>,
);

#[binrw]
#[brw(little, import(version: u32))]
struct Layers(
    #[br(parse_with = until_eof, args(version))]
    #[bw(args(version))]
    Vec<TileBlockLayers>,
);

/// Contents of the `GCTD` chunk.
#[binrw]
#[brw(little)]
struct GrassChunk {
    /// Bytes per block after its coordinates.
    stride: u16,
    #[br(parse_with = until_eof, args(stride.saturating_sub(1)))]
    #[bw(args(stride.saturating_sub(1)))]
    blocks: Vec<TileGrassBlock>,
}

#[binrw::parser(reader, endian)]
fn read_block_index(version: u32) -> BinResult<u32> {
    if version >= TTILE_U32_BLOCK_INDEX_VERSION {
        u32::read_options(reader, endian, ())
    } else {
        u16::read_options(reader, endian, ()).map(u32::from)
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
#[binrw::writer(writer, endian)]
fn write_block_index(index: &u32, version: u32) -> BinResult<()> {
    if version >= TTILE_U32_BLOCK_INDEX_VERSION {
        index.write_options(writer, endian, ())
    } else {
        let pos = writer.stream_position()?;
        u16::try_from(*index)
            .map_err(|e| binrw::Error::Custom {
                pos,
                err: Box::new(e),
            })?
            .write_options(writer, endian, ())
    }
}

impl Ttile {
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
        let heights = read_heights(&required("HGHT")?.data)?;
        let Lods(lods) = parse(&required("BERR")?, "BERR", (version,))?;
        let Layers(layers) = parse(&required("LRS2")?, "LRS2", (version,))?;
        let materials = required("TMAT")?;
        let grass = take("GCTD")
            .map(|c| parse::<GrassChunk, _>(&c, "GCTD", ()))
            .transpose()?
            .map(|g| TileGrass {
                block_clusters: g.stride.saturating_sub(1),
                blocks: g.blocks,
            });

        let materials = read_chunks_in(&materials.data)
            .ok_or(TerrError::InvalidChunk("TMAT"))?
            .iter()
            .map(|c| {
                (&c.tag == b"BMAT")
                    .then(|| read_block_materials(&c.data, version))
                    .flatten()
                    .ok_or(TerrError::InvalidChunk("TMAT"))
            })
            .collect::<Result<_, _>>()?;

        Ok(Self {
            version,
            heights,
            lods,
            grass,
            layers,
            materials,
            unknown_chunks: chunks,
        })
    }

    /// Writes the tile at the current position of `out`. Fields that [`Ttile::version`] does not
    /// store are left out.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let heights = TerrChunk {
            tag: *b"HGHT",
            data: self.heights.iter().flat_map(|h| h.to_le_bytes()).collect(),
        };
        let mut chunks = vec![
            chunk(*b"VERS", &self.version, ())?,
            heights,
            chunk(*b"BERR", &Lods(self.lods.clone()), (self.version,))?,
        ];
        if let Some(grass) = &self.grass {
            if grass
                .blocks
                .iter()
                .any(|b| b.clusters.len() != usize::from(grass.block_clusters))
            {
                return Err(TerrError::Unwritable("GCTD").into());
            }
            let stride = grass
                .block_clusters
                .checked_add(1)
                .ok_or(TerrError::Unwritable("GCTD"))?;
            let grass = GrassChunk {
                stride,
                blocks: grass.blocks.clone(),
            };
            chunks.push(chunk(*b"GCTD", &grass, ())?);
        }
        chunks.push(chunk(
            *b"LRS2",
            &Layers(self.layers.clone()),
            (self.version,),
        )?);

        let mut materials = Cursor::new(Vec::new());
        for block in &self.materials {
            TerrChunk {
                tag: *b"BMAT",
                data: write_block_materials(block, self.version)?,
            }
            .write(&mut materials)?;
        }
        chunks.push(TerrChunk {
            tag: *b"TMAT",
            data: materials.into_inner(),
        });

        write_form(chunks.iter().chain(&self.unknown_chunks), out)
    }

    /// Vertices along one side of the tile, the
    /// [`TerrHead::tile_cells`](crate::enfusion::terr::TerrHead::tile_cells) plus one.
    #[must_use]
    pub const fn vertices(&self) -> u32 {
        self.heights.len().isqrt() as u32
    }

    /// Height map value of the vertex at `x`, `z` in the tile.
    #[must_use]
    pub fn height(&self, x: u32, z: u32) -> Option<u16> {
        let vertices = self.vertices();
        if x >= vertices || z >= vertices {
            return None;
        }
        self.heights.get((z * vertices + x) as usize).copied()
    }
}

fn read_heights(data: &[u8]) -> Result<Vec<u16>, TerrError> {
    let count = data.len() / 2;
    let (heights, rest) = data.as_chunks::<2>();
    if !rest.is_empty() || count.isqrt().pow(2) != count {
        return Err(TerrError::InvalidChunk("HGHT"));
    }
    Ok(heights.iter().map(|h| u16::from_le_bytes(*h)).collect())
}

/// Reads chunks until the end of `data`, which has to be used up exactly.
fn read_chunks_in(data: &[u8]) -> Option<Vec<TerrChunk>> {
    let mut cursor = Cursor::new(data);
    let mut chunks = Vec::new();
    while cursor.position() < data.len() as u64 {
        chunks.push(TerrChunk::read(&mut cursor).ok()?);
    }
    Some(chunks)
}
