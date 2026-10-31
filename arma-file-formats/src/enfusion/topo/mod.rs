//! Enfusion 2D map geometry (`.topo`).
//!
//! Next to a world, written by `MapDataExporter` (`EMapDataType.Geometry2D`) for the in-game
//! map: roads, power lines, buildings, areas, water bodies and hill tops. Coordinates are `x`
//! and `z` in meters, mostly with `z` mirrored, see [`Point`] and [`flip_z`].
//!
//! The file is a tree of chunks, all values little endian. Every chunk has a 20 byte header:
//!
//! ```text
//! char tag[4]; u32 version; u32 size; u32 end; u32 0
//! ```
//!
//! `end` is the offset in the file where the contents end. The file is a `TOPO` chunk (version 1)
//! followed by
//! `_EOF`. `TOPO` holds these chunks, in this order. Lists are a `u32` count and the items,
//! points are two `f32`:
//!
//! | Chunk | Version | Contents                                                              |
//! | ----- | ------- | --------------------------------------------------------------------- |
//! | `ROAD` | 2      | `u32` count per road type, list of levels of detail: list of [`Road`]: `u8` type, points, list of `u32` |
//! | `PWLN` | 1      | list of levels of detail: list of [`PowerLine`]: `u32` type, points   |
//! | `BULD` | 1      | list of [`Buildings`]: list of footprint points, list of [`BuildingPoint`]: `u32`, 4 `f32` |
//! | `AREA` | 1      | twice a list of levels of detail: list of polygon points, then points, see [`Areas`] |
//! | `WATR` | 1      | list of levels of detail: list of [`WaterBody`]: `u32` type, `u8`, points, list of `u32` indices |
//! | `HILL` | 0      | list of [`Hill`]: 6 `f32`, or nothing                                |
//!
//! The game's files have 6 levels of detail and 5 building categories.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::topo::{RoadType, Topo};
//!
//! let topo = Topo::from(&mut BufReader::new(File::open("Arland.topo")?))?;
//! let main_roads = topo.roads.lods[0]
//!     .iter()
//!     .filter(|road| road.kind() == Some(RoadType::RoadMain))
//!     .count();
//! println!("{main_roads} main roads, {} hills", topo.hills.unwrap_or_default().len());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod topo;

pub use self::topo::*;
