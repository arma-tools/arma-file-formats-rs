use std::io::{Read, Seek, Write};

use binrw::{BinWrite, io::NoSeek};

use super::{
    DETOUR_TILE_VERSION, NavTile, NavTileMeta,
    io::{Bytes, PutLe, chunks, len_i32, len_u32, put_chunk, read_form},
    tile::NAV_LINK_SIZE,
};
use crate::{
    core::iff::FormHeader,
    errors::{AffError, NavmeshError},
};

const NTILE_FORM_TYPE: &[u8; 4] = b"NAVD";
const TILE_ENTRY_SIZE: usize = 24;

/// Navmesh tile file (`.ntile`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ntile {
    /// The tile slots of the file, [`NTILES_PER_FILE`](super::NTILES_PER_FILE) in the game's
    /// files. Slots past the end of the grid are `None`.
    pub tiles: Vec<Option<NavTile>>,
}

impl Ntile {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let body = read_form(input, *NTILE_FORM_TYPE)?;
        let [table, data] = chunks(&body, ["TIHE", "DATA"])?;
        // Files of IFF version 2 navmeshes have the same chunks, tell them apart by the version
        // of the first tile, or for files without tiles by the size of the table entries.
        let v2_table =
            !table.len().is_multiple_of(TILE_ENTRY_SIZE) && table.len().is_multiple_of(16);
        if data.get(4..8) == Some(&DETOUR_TILE_VERSION.to_le_bytes()[..])
            || (data.is_empty() && v2_table)
        {
            return Err(NavmeshError::DetourTiles.into());
        }
        Ok(Self::parse(table, data)?)
    }

    fn parse(table: &[u8], data: &[u8]) -> Result<Self, NavmeshError> {
        if !table.len().is_multiple_of(TILE_ENTRY_SIZE) {
            return Err(NavmeshError::InvalidChunk("TIHE"));
        }
        let mut t = Bytes::new(table);
        let mut next_offset = 0;
        let mut tiles = Vec::with_capacity(table.len() / TILE_ENTRY_SIZE);
        for _ in 0..table.len() / TILE_ENTRY_SIZE {
            // The size of the tile once loaded: with room for the links.
            let alloc_size = t.i32()?;
            let max_link_count = t.i32()?;
            let meta_size = t.i32()?;
            let entry_padding = t.u32()?;
            let offset = t.u64()?;
            if alloc_size == 0 {
                if (max_link_count, meta_size, entry_padding, offset) != (0, 0, 0, 0) {
                    return Err(NavmeshError::InvalidChunk("TIHE"));
                }
                tiles.push(None);
                continue;
            }

            let tile_size =
                i64::from(alloc_size) - i64::from(max_link_count) * NAV_LINK_SIZE as i64;
            let (Ok(tile_size), Ok(meta_size), Ok(offset)) = (
                usize::try_from(tile_size),
                usize::try_from(meta_size),
                usize::try_from(offset),
            ) else {
                return Err(NavmeshError::InvalidChunk("TIHE"));
            };
            // The engine seeks to every tile, the writer stores them back to back in slot order.
            if offset != next_offset {
                return Err(NavmeshError::InvalidChunk("TIHE"));
            }
            let mut d = Bytes::new(data.get(offset..).ok_or(NavmeshError::Truncated)?);
            let mut tile = NavTile::parse(d.take(tile_size)?)?;
            if tile.max_link_count != max_link_count {
                return Err(NavmeshError::InvalidTile("link count"));
            }
            if meta_size != 0 {
                tile.meta = Some(NavTileMeta::parse(d.take(meta_size)?)?);
            }
            tile.entry_padding = entry_padding;
            next_offset = offset + tile_size + meta_size;
            tiles.push(Some(tile));
        }
        if next_offset != data.len() {
            return Err(NavmeshError::TrailingData);
        }
        Ok(Self { tiles })
    }

    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let mut table = Vec::with_capacity(self.tiles.len() * TILE_ENTRY_SIZE);
        let mut data = Vec::new();
        for tile in &self.tiles {
            let Some(tile) = tile else {
                table.resize(table.len() + TILE_ENTRY_SIZE, 0);
                continue;
            };
            let offset = data.len();
            tile.write_data(&mut data)?;
            let tile_size = data.len() - offset;
            if let Some(meta) = &tile.meta {
                meta.write(&mut data)?;
            }
            let meta_size = data.len() - offset - tile_size;
            let links = usize::try_from(tile.max_link_count)
                .map_err(|_| NavmeshError::Unwritable("max_link_count"))?;
            table.put_i32(len_i32(tile_size + links * NAV_LINK_SIZE)?);
            table.put_i32(tile.max_link_count);
            table.put_i32(len_i32(meta_size)?);
            table.put_u32(tile.entry_padding);
            table.put_u64(offset as u64);
        }

        let mut body = Vec::with_capacity(table.len() + data.len() + 16);
        put_chunk(&mut body, *b"TIHE", &table)?;
        put_chunk(&mut body, *b"DATA", &data)?;
        FormHeader {
            size: len_u32(body.len() + 4)?,
            form_type: *NTILE_FORM_TYPE,
        }
        .write(&mut NoSeek::new(&mut *out))?;
        out.write_all(&body)?;
        Ok(())
    }

    /// The tiles of the file that exist.
    pub fn iter(&self) -> impl Iterator<Item = &NavTile> {
        self.tiles.iter().flatten()
    }
}
