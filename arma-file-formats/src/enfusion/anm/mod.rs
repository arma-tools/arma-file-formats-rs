//! Enfusion animations (`.anm`).
//!
//! Workbench builds them from `.txa` text animations, which the Blender tools export: per bone a
//! translation, rotation and scale track, each a list of keys on frames. The anm stores every
//! track quantized to `u16`s.
//!
//! An anm is an IFF style `FORM` of type `ANIM` with a single `SET7` chunk that holds the other
//! chunks. Chunk tags and sizes are big endian, everything inside the chunks is little endian.
//! Chunks are not padded to an even size.
//!
//! - `HEAD`: frame count, fps, bone names and per bone the key counts, flags and the
//!   quantization ranges, see [`AnmBone`] and [`AnmTrack`].
//! - `DATA`: per bone the keys of the translation, scale and rotation track, in this order. A
//!   track with keys on some frames starts with the `u16` frame of every key, a track with a key
//!   on every frame stores no frames. Then 3 (4 for rotations) `u16` per key.
//! - `EVN3` (optional): `u16` event count, `u16` size and an EBIN document of the events, the
//!   format of binary worlds, see [`Ent`](crate::enfusion::ent::Ent).
//! - `CPRP` (optional): `u16` count, then pairs of `u32` length and NUL terminated string.
//!
//! Workbench drops keys that interpolation reproduces, [`AnmTrack::sample`] fills them in.
//!
//! Files of Workbench versions until 2025 (mods, BI's samples) have a `SET6` chunk instead, see
//! [`AnmVersion`]. It differs in:
//!
//! - `FPS\0`: a `u32` before `HEAD`.
//! - `HEAD`: no frame count, fps and name list. Per bone the minimum and the range
//!   (`step * 65535`) of every track, `u16` frames, the key counts, flags, a `u8` name length
//!   and the name without NUL. The flags also mark tracks without translation (`0x02`) or
//!   rotation keys (`0x04`).
//! - Events in `EVN1` or `EVN2`, see [`AnmEvents`].
//! - Values are quantized by truncation instead of rounding.
//!
//! `DATA` and `CPRP` are the same. Both versions are read and written.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::anm::Anm;
//!
//! let anm = Anm::from(&mut BufReader::new(File::open("walk.anm")?))?;
//! println!("{} frames at {} fps", anm.frames, anm.fps);
//! if let Some(hips) = anm.bone("Hips") {
//!     for (frame, [x, y, z]) in hips.translation.values() {
//!         println!("key on frame {frame}: {x} {y} {z}");
//!     }
//!     if let Some([x, y, z, w]) = hips.rotation.sample(12.5) {
//!         println!("rotation at frame 12.5: {x} {y} {z} {w}");
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod anm;

pub use self::anm::*;
