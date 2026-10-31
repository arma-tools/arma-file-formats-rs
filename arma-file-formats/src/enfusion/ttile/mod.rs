//! Enfusion terrain tile (`.ttile`).
//!
//! A terrain is split into tiles, `.Data/<name>_<index>.ttile` next to its
//! [`.terr`](crate::enfusion::terr), numbered row by row along x. Each tile is split into blocks,
//! see [`TerrHead`](crate::enfusion::terr::TerrHead). Blocks are stored row by row along x and
//! are addressed by their coordinates in the whole terrain.
//!
//! Like a terr, a tile is an IFF style `FORM` of type `TERR` with big endian chunk tags and sizes
//! and little endian contents:
//!
//! - `VERS` (4 bytes): `u32` data version, the same as the terr's.
//! - `HGHT`: `u16` height map values, see [`Ttile::heights`].
//! - `BERR`: per block height range and level of detail errors, see [`TileBlockLod`]. 36 bytes
//!   per block since version 9, 44 before.
//! - `GCTD` (optional): `u16` stride, the clusters per block plus one, then for each block with
//!   grass its `u16` x and z, the bitwise or of its clusters and the clusters, see [`TileGrass`].
//! - `LRS2`: per block its index, `u16` before version 7 and `u32` since, then a `u16` count
//!   and the `u16` indices of its surface materials, see [`TileBlockLayers`].
//! - `TMAT`: a `BMAT` chunk per block, see [`TileBlockMaterials`]: `u16` x and z, a `u16` count
//!   and the `u16` indices of its materials. If more than one material covers the block a
//!   quadtree follows, see [`MaterialMap`]:
//!   - Since version 9: `u16` node count, `u16` root, four `u16` children per node and, if there
//!     are bitmaps, their `u16` count and for each its `u32` texel count and data.
//!     Children with bit 15 set are materials, with bit 14 set bitmaps, the others nodes.
//!   - Before: a `QTRE` chunk with `u32` node count, root, unknown and root rectangle, four
//!     `u32` children per node, flagged by bits 31 and 30, the `u32` bitmap count and for each
//!     its rectangle, material count, texel count, `u32` count and the `u32`s.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::{terr::Terr, ttile::Ttile};
//!
//! let terr = Terr::from(&mut BufReader::new(File::open("Terrain.terr")?))?;
//! let tile = Ttile::from(&mut BufReader::new(File::open(".Data/Terrain_0.ttile")?))?;
//! let height = tile.height(0, 0).map(|h| terr.head.height(h));
//! println!("{height:?} m");
//! for block in &tile.materials {
//!     let material = block.material_at(tile.version, 0, 0, 64);
//!     println!("{}, {}: {:?}", block.x, block.z, material.map(|m| &terr.materials[m as usize]));
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod materials;
#[allow(clippy::module_inception)]
mod ttile;

pub use self::{materials::*, ttile::*};
