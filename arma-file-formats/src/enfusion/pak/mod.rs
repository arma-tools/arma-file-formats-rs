//! Enfusion archive (`.pak`).
//!
//! A pak is an IFF style `FORM` of type `PAC1` with three chunks. Chunk tags and sizes are big
//! endian, everything inside the chunks is little endian.
//!
//! - `HEAD` (28 bytes): `u32` version `0x0001_0003`, the CRC32 of the `FILE` chunk data, the
//!   same CRC formatted as `"{crc:08x}{crc:08x}"` XOR `"BIStudio19092012"` and a zero `u32`.
//! - `DATA`: the stored file data, back to back in the order of the `FILE` tree.
//! - `FILE`: the directory tree. Each node is a `u8` kind (`0` directory, `1` file) and a `u8`
//!   length prefixed UTF-8 name. A directory continues with a `u32` child count and its children,
//!   sorted by their ASCII uppercased names. A file continues with its absolute data offset,
//!   compressed size, size, six zero bytes, the compression (`0` stored, `1` zlib), the zlib
//!   level and a [`PakTimestamp`]. The root is a directory with an empty name.
//!
//! # Reading
//!
//! [`Pak`] holds the table of contents. [`PakReader`] keeps the stream next to it and reads file
//! data only when an entry is read or opened. Opened entries are streams that decompress while
//! they are read. Any number can be open at once, they share the underlying stream.
//!
//! ```no_run
//! use std::io::{self, Read};
//!
//! use arma_file_formats::enfusion::pak::PakReader;
//!
//! let reader = PakReader::from_path("data.pak")?;
//! for entry in &reader.pak.entries {
//!     println!("{} ({} bytes, {})", entry.path, entry.size, entry.timestamp);
//! }
//!
//! // Lookup is ASCII case insensitive and accepts `\`.
//! let config = reader.read("Configs\\game.conf")?;
//!
//! // Stream a large file instead of reading it into memory.
//! if let Some(mut stream) = reader.open("Worlds/terrain.ent")? {
//!     io::copy(&mut stream, &mut io::sink())?;
//! }
//! # Ok::<(), arma_file_formats::errors::AffError>(())
//! ```
//!
//! # Writing
//!
//! [`PakWriter`] orders files like Workbench, so packing the same files gives the same layout.
//! File contents come from a [`PakSource`]: bytes, a path or a reader. Paths and readers are only
//! read in [`PakWriter::write`], one file at a time, and stored or compressed data is streamed to
//! the output. The output has to be seekable because the sizes and the checksum are written last.
//!
//! [`PakWriter::add_file`] compresses at [`PAK_DEFAULT_ZLIB_LEVEL`] when that makes the file
//! smaller, which matches the choices Workbench makes. It holds the file in memory to decide.
//! [`PakWriter::add_file_with_compression`] streams with a fixed compression.
//!
//! ```no_run
//! use std::{fs::File, io::BufWriter, path::Path};
//!
//! use arma_file_formats::enfusion::pak::{PakCompression, PakTimestamp, PakWriter};
//!
//! let timestamp = PakTimestamp::new(2026, 10, 30, 12, 0, 0).unwrap();
//! let mut writer = PakWriter::new();
//! writer.add_file("Configs/game.conf", b"Game {\n}\n", timestamp)?;
//! writer.add_file_with_compression(
//!     "Worlds/terrain.ent",
//!     Path::new("addon/Worlds/terrain.ent"),
//!     timestamp,
//!     PakCompression::Stored,
//! )?;
//! writer.write(&mut BufWriter::new(File::create("data.pak")?))?;
//! # Ok::<(), arma_file_formats::errors::AffError>(())
//! ```
//!
//! # Repacking
//!
//! [`PakWriter::add_raw`] copies an entry of another pak without recompressing it. Streaming the
//! raw entries of a [`PakReader`] repacks a pak of any size in constant memory, and the result is
//! byte identical to a pak packed by Workbench.
//!
//! ```no_run
//! use std::{fs::File, io::BufWriter};
//!
//! use arma_file_formats::enfusion::pak::{PakReader, PakSource, PakWriter};
//!
//! let reader = PakReader::from_path("data.pak")?;
//! let mut writer = PakWriter::new();
//! for entry in &reader.pak.entries {
//!     if !entry.path.starts_with("Unused/") {
//!         writer.add_raw(entry, PakSource::reader(reader.open_entry_raw(entry)?))?;
//!     }
//! }
//! writer.write(&mut BufWriter::new(File::create("repacked.pak")?))?;
//! # Ok::<(), arma_file_formats::errors::AffError>(())
//! ```

mod entry;
mod file_tree;
#[allow(clippy::module_inception)]
mod pak;
mod timestamp;
mod writer;

pub use self::{entry::*, pak::*, timestamp::*, writer::*};
