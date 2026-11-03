//! Enfusion fonts (`.fnt`).
//!
//! A font is a glyph atlas texture, the `.edds` with the same name next to the `.fnt`, and this
//! file: which code points it has, where their glyphs are in the atlas and how to place them.
//! Workbench's Font Editor renders a TrueType font into both. Bitmap fonts hold the glyphs as
//! they are, distance field fonts (SDF, or multi channel MSDF) hold a distance field that the
//! engine scales, outlines and shadows at runtime.
//!
//! A font is a `FORM` of type `FNT2` to `FNT5` (the version), chunk tags and sizes are big
//! endian, everything inside the chunks is little endian. Chunks can come in any order, see
//! [`Fnt::chunk_order`]. Version 1 (`FONT`) is not supported by the engine either.
//!
//! - `HEAD`: the font name and size, from version 4 the type and SDF metrics, see [`FntHead`].
//! - `GLPS`: the glyph count and the code point ranges, see [`FntRange`]. A range is a `u16`
//!   start and count before version 5, a `u32` start, `u16` count and 2 bytes of padding since.
//! - `TCRD`: one [`FntGlyph`] per code point, in range order: 8 bytes in version 2, 12 in version
//!   3, 14 since version 4.
//! - `LYRS` (optional): [`FntColorGlyph`]s, layered emoji. Not used by the game's fonts.
//! - `KERN`: [`FntKerning`] pairs, empty in all of the game's fonts.
//!
//! The Font Editor writes `FNT5` for distance field fonts and `FNT3` for bitmap fonts.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::fnt::Fnt;
//!
//! let fnt = Fnt::from(&mut BufReader::new(File::open("UI/Fonts/Roboto/Roboto_Regular.fnt")?))?;
//! if let Some(glyph) = fnt.glyph('A' as u32) {
//!     let [x0, y0, x1, y1] = fnt.atlas_rect(glyph);
//!     println!("'A' is at {x0}, {y0} to {x1}, {y1} in the atlas");
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod fnt;

pub use self::fnt::*;
