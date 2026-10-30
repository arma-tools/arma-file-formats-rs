//! Enfusion terrain descriptor (`.terr`).
//!
//! The `.terr` file holds the terrain wide settings. The terrain data itself is split into tiles
//! that live next to it in `.Data/<name>_<index>.ttile`, each with optional
//! `_supertexture.edds`, `_normal.edds` and `_layer.edds` textures.
//!
//! A terr is an IFF style `FORM` of type `TERR`. Chunk tags and sizes are big endian, everything
//! inside the chunks is little endian.
//!
//! - `VERS` (4 bytes): `u32` data version, `5`, `7` and `9` are known.
//! - `HEAD` (32 bytes): the grid, see [`TerrHead`].
//! - `TEXS` (6 bytes): per tile texture resolutions, see [`TerrTextureSizes`].
//! - `MATS`: since version 9 a `u16` [`Terr::surface_bits`], then the surface materials until
//!   the end of the chunk. Each is a `u32` length and a NUL terminated resource path, the length
//!   includes the NUL.
//! - `GRAS` (6 bytes): grass clusters, see [`TerrGrass`].
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::terr::Terr;
//!
//! let terr = Terr::from(&mut BufReader::new(File::open("Terrain.terr")?))?;
//! let [x, z] = terr.head.size_meters();
//! println!("{x}x{z} m, {} tiles", terr.head.tile_count());
//! for material in &terr.materials {
//!     println!("{material}");
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod terr;

pub use self::terr::*;
