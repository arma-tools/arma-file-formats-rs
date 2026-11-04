use std::io::{Read, Seek, Write};

use binrw::{BinWrite, io::NoSeek};

use super::io::{Bytes, PutLe, chunks, len_u32, put_chunk, read_form};
use crate::{
    core::iff::FormHeader,
    errors::{AffError, NavmeshError},
};

/// First 4 bytes of an `.nmn`: `EV`, `2`, `0`.
pub const NMN_MAGIC: u32 = 0x0002_5645;
/// The only IFF version the engine reads.
pub const NMN_IFF_VERSION: u32 = 3;
/// The previous IFF version, whose tiles are [`NtileV2`](super::NtileV2) files. Six of the game's
/// navmeshes still have it, the engine refuses to load them.
pub const NMN_IFF_VERSION_2: u32 = 2;
/// Longest [`Nmn::path`] the engine reads.
pub const NMN_MAX_PATH_LEN: usize = 0x118;
/// Tiles per `.ntile` file the engine writes.
pub const NTILES_PER_FILE: u32 = 100;

const NMN_FORM_TYPE: &[u8; 4] = b"NAVM";
const TILE_ENTRY_SIZE: usize = 16;

/// Navmesh main file (`.nmn`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq)]
pub struct Nmn {
    /// [`NMN_IFF_VERSION`], or [`NMN_IFF_VERSION_2`] for navmeshes with [`NtileV2`](super::NtileV2)
    /// tile files.
    pub iff_version: u32,
    pub params: NavMeshParams,
    /// The engine reads up to [`NMN_MAX_PATH_LEN`] bytes here, but always writes an empty
    /// string.
    pub path: String,
    /// The 2 bytes after the `u16` length of [`Nmn::path`]. The engine leaves them uninitialized
    /// when writing (`0x900d` in the game's files). Kept for byte exact round trips, use `0` for
    /// new files.
    pub path_padding: u16,
    /// Where the tiles are, [`NavMeshParams::max_tiles`] entries in storage order: entry `i` is
    /// slot `i % NTILES_PER_FILE` of `.ntile` file `i / NTILES_PER_FILE`. The tiles are not in
    /// grid order (the engine looks them up by position), navmeshes without tiles in some
    /// places have fewer tiles than entries and `0, 0` in the entries after the last tile.
    pub tiles: Vec<NmnTile>,
}

/// The navmesh's tile grid, Detour's `dtNavMeshParams`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavMeshParams {
    /// World position of the corner of tile `0, 0`.
    pub origin: [f32; 3],
    /// Tile size along x in meters.
    pub tile_width: f32,
    /// Tile size along z in meters.
    pub tile_height: f32,
    /// Most tiles, a square number in the game's navmeshes. Together with
    /// [`NavMeshParams::max_polys`] it determines the bits of a polygon reference.
    pub max_tiles: u32,
    /// Most polygons per tile.
    pub max_polys: u32,
}

/// Position and storage place of a tile of an [`Nmn`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NmnTile {
    /// Tile column, the tile's x is `origin + x * tile_width`. `0, 0` in entries after the last
    /// tile. Can be negative.
    pub x: i32,
    /// Tile row, along z.
    pub y: i32,
    /// Number `N` of the `navData_N.ntile` file.
    pub file: u32,
    /// Slot within the `.ntile` file.
    pub index: u32,
}

impl Nmn {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let mut header = [0; 40];
        input.read_exact(&mut header)?;
        let mut h = Bytes::new(&header);
        let magic = h.u32()?;
        if magic != NMN_MAGIC {
            return Err(NavmeshError::UnknownMagic(magic).into());
        }
        let iff_version = h.u32()?;
        if iff_version != NMN_IFF_VERSION && iff_version != NMN_IFF_VERSION_2 {
            return Err(NavmeshError::UnsupportedVersion(iff_version).into());
        }
        let params = NavMeshParams {
            origin: h.vec3()?,
            tile_width: h.f32()?,
            tile_height: h.f32()?,
            max_tiles: h.u32()?,
            max_polys: h.u32()?,
        };
        let path_len = usize::from(h.u16()?);
        let path_padding = h.u16()?;
        if path_len > NMN_MAX_PATH_LEN {
            return Err(NavmeshError::PathTooLong(path_len).into());
        }
        let mut path = vec![0; path_len];
        input.read_exact(&mut path)?;
        let path = String::from_utf8(path).map_err(|_| NavmeshError::InvalidPath)?;

        let body = read_form(input, *NMN_FORM_TYPE)?;
        let [table] = chunks(&body, ["TFHE"])?;
        if !table.len().is_multiple_of(TILE_ENTRY_SIZE) {
            return Err(NavmeshError::InvalidChunk("TFHE").into());
        }
        let mut t = Bytes::new(table);
        let tiles = (0..table.len() / TILE_ENTRY_SIZE)
            .map(|_| {
                Ok(NmnTile {
                    x: t.i32()?,
                    y: t.i32()?,
                    file: t.u32()?,
                    index: t.u32()?,
                })
            })
            .collect::<Result<_, NavmeshError>>()?;

        Ok(Self {
            iff_version,
            params,
            path,
            path_padding,
            tiles,
        })
    }

    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        if self.path.len() > NMN_MAX_PATH_LEN {
            return Err(NavmeshError::PathTooLong(self.path.len()).into());
        }
        let mut header = Vec::with_capacity(40 + self.path.len());
        header.put_u32(NMN_MAGIC);
        header.put_u32(self.iff_version);
        header.put_vec3(self.params.origin);
        header.put_f32(self.params.tile_width);
        header.put_f32(self.params.tile_height);
        header.put_u32(self.params.max_tiles);
        header.put_u32(self.params.max_polys);
        header.put_u16(self.path.len() as u16);
        header.put_u16(self.path_padding);
        header.extend_from_slice(self.path.as_bytes());

        let mut table = Vec::with_capacity(self.tiles.len() * TILE_ENTRY_SIZE);
        for tile in &self.tiles {
            table.put_i32(tile.x);
            table.put_i32(tile.y);
            table.put_u32(tile.file);
            table.put_u32(tile.index);
        }
        let mut body = Vec::new();
        put_chunk(&mut body, *b"TFHE", &table)?;

        out.write_all(&header)?;
        FormHeader {
            size: len_u32(body.len() + 4)?,
            form_type: *NMN_FORM_TYPE,
        }
        .write(&mut NoSeek::new(&mut *out))?;
        out.write_all(&body)?;
        Ok(())
    }

    /// Number of `.ntile` files, the highest [`NmnTile::file`] plus one.
    #[must_use]
    pub fn file_count(&self) -> u32 {
        self.tiles.iter().map(|t| t.file + 1).max().unwrap_or(0)
    }

    /// The first entry of the tile at `x, y`. Searches all entries, build a map from
    /// [`Nmn::tiles`] for many lookups. For `0, 0` this can be an entry after the last tile,
    /// whose `.ntile` slot is empty.
    #[must_use]
    pub fn tile(&self, x: i32, y: i32) -> Option<&NmnTile> {
        self.tiles.iter().find(|t| t.x == x && t.y == y)
    }

    /// The tile column and row of a world position (x and z).
    #[must_use]
    pub fn tile_at(&self, x: f32, z: f32) -> (i32, i32) {
        let p = &self.params;
        (
            ((x - p.origin[0]) / p.tile_width).floor() as i32,
            ((z - p.origin[2]) / p.tile_height).floor() as i32,
        )
    }

    /// Path of `.ntile` file `file` of the navmesh at `nmn_path`: `<dir>/.NavData/<name>/navData_<file>.ntile`.
    #[must_use]
    pub fn ntile_path(nmn_path: &str, file: u32) -> String {
        let (dir, name) = nmn_path.rsplit_once('/').unwrap_or(("", nmn_path));
        let name = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
        let dir = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        format!("{dir}.NavData/{name}/navData_{file}.ntile")
    }
}
