//! Enfusion sound map (`.smap`).
//!
//! Next to a world, referenced by the `Soundmap` of its `SoundWorld` entity: what the ambient
//! sounds of a place are, e.g. sea, forest or city. There are two layouts, see [`Smap`].
//!
//! [`SoundMapGrid`], the layout `MapDataExporter` (`EMapDataType.Soundmap`) writes, is little
//! endian without chunks:
//!
//! ```text
//! u32 cells; u32 cells_z; u32 cells_x; f32 cell_size; f32 min[3]; f32 max[3];
//! { i32 terrain; i32 freshwater; } cells[cells];
//! 2 x { u32 records; u32 indices; u32 id; record[records]; u16 index[indices]; }
//! ```
//!
//! The first layer (id `0`) has a 6 byte [`TerrainSound`] per cell, the second (id `1`) a 4 byte
//! [`FreshwaterSound`] per 2 m sub-cell of the cells near fresh water.
//!
//! [`SoundMapLayers`] is an IFF style `FORM` of type `SMAP` with big endian chunk tags and sizes
//! and little endian contents:
//!
//! - `VERS` (4 bytes): `u32` version, only `1` is known.
//! - `HEAD` (28 bytes): `f32` min and max `x`, `y`, `z` and a `u32`.
//! - A chunk per layer, `L$10` for a `SoundMapLayerGrass`, see [`SoundLayer`]:
//!   - `HEAD`: a `VER` chunk with a `u16` version (`1`), `f32` min and max, `u32` block size and
//!     tile size in meters, `u16` blocks along `x` and `z`.
//!   - `BLKS`: a `BLCK` chunk per block, see [`SoundBlock`].
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::smap::Smap;
//!
//! if let Smap::Grid(grid) = Smap::from(&mut BufReader::new(File::open("Arland.smap")?))? {
//!     let sound = grid.terrain_at([1000.0, 2000.0]);
//!     println!("{sound:?}");
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod smap;

pub use self::smap::*;
