//! Enfusion star catalog (`.stars`).
//!
//! The night sky: the game ships one, `system/star_database.stars` (`{B5A8E14A07CD21BB}`), with
//! 128,503 entries for the 117,942 stars of the Hipparcos catalogue. The other 10,561 are
//! copies with the same data, named (`CAPELLA A`) or not, up to 19 for one star. The star dome
//! loads the first `Number of stars` entries (1 to 100,000) of the sky entity, so they are
//! sorted by magnitude, brightest first. Without the file it falls back to random stars.
//!
//! A catalog is an IFF style `FORM` of type `STAR`. Chunk tags and sizes are big endian,
//! everything else is little endian, and chunks are not padded.
//!
//! - `u32` star count, right after the form type, outside of any chunk.
//! - One `ITEM` chunk per star, see [`Star`]: six `f32` (right ascension, declination, proper
//!   motion, parallax, magnitude), the `u32` colour, the `u32` Hipparcos number and the name as
//!   a `u32` length including the NUL, then the bytes and the NUL.
//!
//! The engine reads only the floats and the colour, and skips other chunks.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::stars::StarCatalog;
//!
//! let catalog = StarCatalog::from(&mut BufReader::new(File::open("star_database.stars")?))?;
//! for star in catalog.stars.iter().filter(|s| !s.name.is_empty()).take(10) {
//!     println!("{} at {:.3} h, {:.3} deg, magnitude {}", star.name, star.right_ascension,
//!         star.declination, star.magnitude);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod stars;

pub use self::stars::*;
