//! Enfusion binary world (`.ent`).
//!
//! The worlds in the game's PAKs are binary, one file holding all layers. Worlds in addon
//! sources are text instead, with the entities in `<World>_Layers/*.layer` files. The binary
//! layout is a token stream that mirrors the text: keys, values and nested blocks.
//!
//! All values are little endian:
//!
//! ```text
//! "EBIN"
//! u32  version                    1
//! u16  string count
//! u16  GUID count
//! strings                         u16 length + UTF-8: keys, class names, `$grp`, `:`
//! u64  GUIDs                      resource GUIDs and shared object ids
//! tokens until the end of the file
//! ```
//!
//! Both tables are in the order of first use. Every token is a `u8` tag and its payload:
//!
//! | Tag        | Payload                         | Meaning                                       |
//! | ---------- | ------------------------------- | --------------------------------------------- |
//! | `0d`       | `u16` string index              | key or class name, `:` before a parent        |
//! | `0e`       | `u32` size (including itself)   | a block, see [`Body`]                         |
//! | `00`       | `u16`                           | header of an object instance's body, see [`Body::Entries`] |
//! | `43`       | `u16` length + UTF-8            | entity name, [`Object::name`]                 |
//! | `44`       | `u64`                           | [`ObjectId::Inline`]                          |
//! | `45`       | `u16` GUID index                | [`ObjectId::Shared`]                          |
//! | `05`       | `u16` GUID index                | [`Value::Resource`]                           |
//! | `01`       | `i32`                           | [`Value::Int`]                                |
//! | `02`       | `u64`                           | [`Value::Flags`]                              |
//! | `06`       | `f32`                           | [`Value::Float`]                              |
//! | `07` - `09` | 2 to 4 `f32`                   | [`Value::Vec2`] to [`Value::Vec4`]            |
//! | `0b`       | `u8`                            | [`Value::Bool`]                               |
//! | `0c`       | `u16` length + UTF-8            | [`Value::String`]                             |
//! | `80 \| t`  | `u32` count + count payloads of `t` | arrays, e.g. `85` resources, `8c` strings |
//!
//! A property is a key and one value. An object is an optional key and class, an optional
//! name, an optional id, optionally `:` and the resource it inherits from and then a block, see
//! [`Object`]. A world of a prefab and a group of two entities, one with a child:
//!
//! ```text
//! SCR_AIWorld : "{E0A05C76552E7F58}" {         0d 0d(:) 05 0e( 00
//!  coords 107.9 1 159.5                          0d 08 )
//! }
//! $grp GenericEntity {                          0d 0d 0e(
//!  First {                                       43 0e( 00
//!   coords 1 2 3                                  0d 08
//!   {                                             0e(
//!    GenericEntity Child {                         0d 43 0e( 00 )
//!    }
//!   }                                             )
//!  }                                             )
//!  {                                             0e( 00
//!   Filenames {                                   0d 0e(
//!    "{5EEBC743F318F376}"                          85 )
//!   }                                             )
//!  }
//! }                                             )
//! ```
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::ent::Ent;
//!
//! let world = Ent::from(&mut BufReader::new(File::open("EmptyArland.ent")?))?;
//! for entity in world.entities() {
//!     println!("{} {:?} at {:?}", entity.class, entity.name, entity.object.coords());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod binary;
#[allow(clippy::module_inception)]
mod ent;

pub use self::{binary::ENT_MAGIC, ent::*};
