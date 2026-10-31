//! Enfusion shore map data (`.smd`).
//!
//! Next to a world with an ocean, written by `WorldEditorAPI.BuildShoreMap` with the settings in
//! `<world>_SMD.conf`: which parts of the map have shore textures, and where they are in the
//! atlases of `<world dir>/.Shore/`: `shoreDFAtlas.edds` (RGBA), `shoreDFNearAtlas.edds`,
//! `shoreMaskAtlas.edds` and optionally `shoreFoamMaskAtlas.edds` (BC4). The map is split into
//! square tiles, every tile near the coast has a slot in the atlases.
//!
//! An smd is an IFF style `FORM` of type `SMDF`. Chunk tags and sizes are big endian, everything
//! inside the chunks is little endian.
//!
//! - `VERS` (4 bytes): `u32` version, only `2` is known.
//! - `HEAD` (44 bytes): extent, tile and atlas sizes, see [`SmdHead`].
//! - `TMAP`: `u32` width and height in tiles, then a `u16` per tile, see [`SmdTileMap`].
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::smd::Smd;
//!
//! let smd = Smd::from(&mut BufReader::new(File::open("Arland.smd")?))?;
//! if let Some(index) = smd.tile_at([1000.0, 2000.0]) {
//!     let [x, y] = smd.head.atlas_slot(index);
//!     println!("shore tile {index}, atlas pixels from {x}, {y}");
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod smd;

pub use self::smd::*;
