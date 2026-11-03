//! Enfusion resource database (`resourceDatabase.rdb`).
//!
//! Workbench's cache of an addon's file tree, saved next to the `.gproj` whenever it scans the
//! addon. Packed addons ship it next to their paks, and the engine uses it instead of scanning
//! them (the paks hold no `.meta` files). It maps every resource GUID to its path, which
//! [`ResourceRef`](crate::enfusion::resource::ResourceRef)s need when their path is outdated or
//! missing.
//!
//! An rdb is a `FORM` of type `RDBC` without chunks: the form size is big endian, everything
//! after the form type is little endian. Strings are a `u32` length including the NUL, then
//! the bytes and the NUL.
//!
//! - `u32` version, only `7` is known, and the `u64` size of the whole file.
//! - `u32` tag count and the tag strings, see [`Rdb::tags`].
//! - `u32` entry count and the entries, sorted by their lowercased path, see [`RdbEntry`]: the
//!   path, `u16` flags, `u32` tag mask, `u64` GUID and, with a `.meta`, its [`PakTimestamp`]
//!   and `u32` size.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::{rdb::Rdb, resource::ResourceRef};
//!
//! let rdb = Rdb::from(&mut BufReader::new(File::open("resourceDatabase.rdb")?))?;
//! let reference = ResourceRef::parse("{3C10D749A4E84B38}Common/Materials/Old.emat").unwrap();
//! if let Some(entry) = rdb.resolve(&reference) {
//!     println!("{} has the tags {:?}", entry.path, rdb.entry_tags(entry).collect::<Vec<_>>());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [`PakTimestamp`]: crate::enfusion::pak::PakTimestamp

#[allow(clippy::module_inception)]
mod rdb;

pub use self::rdb::*;
