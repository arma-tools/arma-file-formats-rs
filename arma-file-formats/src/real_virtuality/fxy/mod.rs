//! Real Virtuality fonts (`.fxy`).
//!
//! A font of one size is a `.fxy` and its texture pages next to it: `Purista10.fxy` has its
//! glyphs in `Purista10-01.paa`, `Purista10-02.paa` and so on, see [`Fxy::texture_path`]. The
//! `.fxy` says per character on which page its glyph is, where, and how to place it.
//! `FontToTGA` of the Arma 3 Tools renders a Windows font into `.tga` pages (made `.paa` later)
//! and the `.fxy`, the ranges of characters come from its `FontToTga.cfg`.
//!
//! Little endian, `BIFo` and a `u32` version, then:
//!
//! - Version 1 (`0x101`): [`FxyCell`]s up to the end of the file, 14 bytes each: the character
//!   minus `0x20`, the page and the glyph's rectangle on it as `u16`, and its advance. The
//!   rectangle starts at the top of the line and at the pen position.
//! - Version 2 (`0x102`, what `FontToTGA` writes): [`FxyPage`]s up to the end of the file, one
//!   per texture page: the page number (from `1`), the line height and ascent as `i32`, a `u32`
//!   count and the [`FxyKerning`] pairs of the characters on the page, a `u32` count and the
//!   page's [`FxyGlyph`]s, 22 bytes each. Glyph rectangles are tight, the offsets place them
//!   relative to the pen position and the top of the line.
//!
//! The Arma 3 base game has 448 fonts (`uifonts_f_data.pbo`), 178 in version 1 and 270 in
//! version 2. All of them read and write byte exact.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::real_virtuality::fxy::Fxy;
//!
//! let fxy = Fxy::from(&mut BufReader::new(File::open("Purista10.fxy")?))?;
//! if let Some(glyph) = fxy.glyph('A' as u32) {
//!     let texture = Fxy::texture_path("Purista10.fxy", glyph.page);
//!     let (x, y) = (glyph.x, glyph.y);
//!     println!("'A' is {}x{} at {x}, {y} of {texture}", glyph.width, glyph.height);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod fxy;

pub use self::fxy::*;
