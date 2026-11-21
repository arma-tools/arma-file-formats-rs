//! Real Virtuality texture headers (`texHeaders.bin`).
//!
//! Binarize of the Arma 3 Tools writes one into each PBO with textures (`-texheader` makes only
//! that file): per `.paa` and `.pac` of the folder the header the engine would otherwise read from
//! the texture, so it doesn't have to open every texture when loading. Little endian:
//!
//! - `0DHT`, `u32` version (`1`), `u32` texture count.
//! - Per texture a [`TexHeader`]: palette fields, the average color as 4 `f32` and the max color
//!   (from the `AVGC` and `MAXC` tags of the texture), alpha flags (from its `FLAG` tag), the
//!   mipmap count and pixel format, the path relative to the PBO (lower case, NUL terminated),
//!   the [`TextureType`] (from the file name), and per mipmap its size, format and position in the
//!   texture. Last the size of the texture file.
//!
//! [`TexHeader::from_paa`] makes the header of a texture like Binarize, and
//! [`TexHeaders::from_textures`] puts the headers of a folder in Binarize's order: the order of a
//! hash table of the paths, see [`TexHeaders::sort_like_binarize`].
//!
//! Arma 3 has 85 `texHeaders.bin` with 19,591 textures. All of them round trip byte exact, and
//! built from the PBO's textures with [`TexHeaders::from_textures`] they give the game's files.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::real_virtuality::texheaders::TexHeaders;
//!
//! let headers = TexHeaders::from(&mut BufReader::new(File::open("texHeaders.bin")?))?;
//! for texture in &headers.textures {
//!     println!("{} {:?} {:?}", texture.path, texture.pixel_format, texture.texture_type);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod texheaders;

pub use self::texheaders::*;
