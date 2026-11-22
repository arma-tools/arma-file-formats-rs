//! Real Virtuality compiled scripts (`.sqfc`).
//!
//! Arma 3 loads a `fn_name.sqfc` next to `fn_name.sqf` instead of compiling the script. It holds
//! the bytecode of the script's stack machine and the preprocessed script itself, which
//! [`Sqfc::source`] gives back.
//!
//! Little endian, a `u32` version, then blocks of a type byte and their content:
//!
//! - `5`, the name table ([`Sqfc::name_table`], optional): the command and variable names of the
//!   instructions, a `u16` count and the strings. Compressed.
//! - `1`, the [`SqfcConstant`]s, compressed (or `0`, uncompressed): a `u16` count and the
//!   constants, each a type byte and its value. Code blocks are constants too, a `u64` that says
//!   where their source text is ([`SqfcCodeSource`]), a `u32` count and the
//!   [`SqfcInstruction`]s. Instructions are an opcode, the [`SqfcLocation`] (all but
//!   [`SqfcInstruction::EndStatement`] and [`SqfcInstruction::Push`]) and the argument: a
//!   constant index, an index into the name table (or the name), or an array size.
//! - `2`, the files: a `u16` count and the paths the script was preprocessed from.
//! - `3`, the code: the `u64` index of the constant that runs, see [`Sqfc::code`].
//!
//! Strings are a 3 byte length and the bytes. A compressed block is the `u32` size of the
//! uncompressed bytes, `2` for LZO1X and the LZO stream, which ends with its end marker.
//! Version 2 adds hash maps and a flags byte to arrays, it is described by the public
//! `ArmaScriptCompiler` but no game or mod file uses it.
//!
//! The Arma 3 base game has 1,295 scripts (`functions_f.pbo`, `ui_f.pbo`), all version 1. All of
//! them read and write byte exact. Of 5,974 scripts in mods all read, 1,862 write byte exact, the
//! others are HEMTT's, whose LZO encoder differs: their blocks decompress to the same bytes.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::real_virtuality::sqfc::{Sqfc, SqfcInstruction};
//!
//! let sqfc = Sqfc::from(&mut BufReader::new(File::open("fn_spawnGroup.sqfc")?))?;
//! println!("{}", sqfc.source().unwrap_or_default());
//! for instruction in &sqfc.code().unwrap().instructions {
//!     if let SqfcInstruction::CallBinary { name, location } = instruction {
//!         println!("{name} at line {}", location.line);
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod sqfc;

pub use self::sqfc::*;
