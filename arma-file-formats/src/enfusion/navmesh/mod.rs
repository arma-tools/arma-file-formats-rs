//! Enfusion navmeshes (`.nmn` and `.ntile`), the AI navigation data of a world.
//!
//! Workbench builds them with Recast & Detour: one navmesh per agent type (soldier, vehicle
//! sizes, ...), each a grid of tiles. `<dir>/<name>.nmn` describes the grid and lists the tiles,
//! the tiles are in `<dir>/.NavData/<name>/navData_<N>.ntile`, 100 per file, see
//! [`Nmn::ntile_path`].
//!
//! Everything is little endian except `FORM` and chunk headers. Decoded from the game's files
//! and the engine's loader, all 39 `.nmn` and 93,337 `.ntile` files of the game read and write
//! back byte exact.
//!
//! An `.nmn` starts with a header before its `FORM`: the magic [`NMN_MAGIC`] (`EV`), the IFF
//! version [`NMN_IFF_VERSION`], Detour's `dtNavMeshParams` ([`NavMeshParams`]) and a string
//! with a `u16` length and 2 bytes of padding ([`Nmn::path`], always empty). Then a `FORM` of
//! type `NAVM` with one chunk:
//!
//! - `TFHE`: per tile 16 bytes, [`NmnTile`]: `x`, `y`, `.ntile` file, slot in the file.
//!
//! An `.ntile` is a `FORM` of type `NAVD` with two chunks:
//!
//! - `TIHE`: per tile slot 24 bytes: `i32` tile size plus 16 bytes per link (the size the
//!   engine allocates), `i32` link count ([`NavTile::max_link_count`]), `i32` meta data size,
//!   4 bytes padding ([`NavTile::entry_padding`]), `u64` offset in `DATA`. All zero for an
//!   empty slot.
//! - `DATA`: the tiles, each followed by its meta data.
//!
//! A tile ([`NavTile`]) is Detour's tile data from the engine's modified Detour (version
//! [`NAV_TILE_VERSION`] instead of 7):
//!
//! - A 92 byte header: magic `DNAV` (`VAND` in the file), version, `x`, `y`, layer, user ID,
//!   the polygon, vertex, edge, link, BV node and off-mesh connection counts, the off-mesh
//!   base, the agent's height, radius and climb, `bmin`, `bmax` and the BV quantization factor.
//! - Polygon edges, `u16` vertex and `u16` neighbour ([`NavPolyEdge`]).
//! - Vertices, three `u16` each, quantized between `bmin` and `bmax`, padded to 4 bytes.
//! - Polygons, 12 bytes ([`NavPoly`]): first link, first edge, flags, edge count and area.
//! - BV tree nodes ([`NavBvNode`]) and off-mesh connections ([`NavOffMeshConnection`]) as in
//!   Detour. Detour's links (inserted after the polygons when the engine loads the tile) and
//!   detail meshes are not stored.
//! - The meters per vertex quantization step ([`NavTile::vertex_scale`]) and `DNAV` again.
//!
//! The meta data ([`NavTileMeta`]) is Enfusion's own: version, `x`, `y`, the custom links
//! (doors, ladders, ...) and the cover edges, each a count and a size.
//!
//! Older versions the engine no longer loads are still in the game's files and supported too:
//! tiles of version [`NAV_TILE_VERSION_10`] (without the final `DNAV`), and navmeshes of IFF
//! version [`NMN_IFF_VERSION_2`], whose tile files ([`NtileV2`]) hold stock Detour tiles
//! ([`DetourTile`]).
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::navmesh::{Nmn, Ntile};
//!
//! let path = "worlds/SP/Tutorial/Tutorial_Navmesh_Soldier.nmn";
//! let nmn = Nmn::from(&mut BufReader::new(File::open(path)?))?;
//! let (x, y) = nmn.tile_at(2020.0, 1430.0);
//! let slot = nmn.tile(x, y).expect("a tile there");
//! let ntile = Ntile::from(&mut BufReader::new(File::open(Nmn::ntile_path(path, slot.file))?))?;
//! if let Some(tile) = &ntile.tiles[slot.index as usize] {
//!     println!("{} polygons, first vertex at {:?}", tile.polys.len(), tile.vertex_position(0));
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod detour;
mod io;
mod meta;
mod nmn;
mod ntile;
mod tile;

pub use self::{detour::*, meta::*, nmn::*, ntile::*, tile::*};
