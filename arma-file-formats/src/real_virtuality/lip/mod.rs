//! Real Virtuality lip sync (`.lip`).
//!
//! Next to a spoken sound (`.ogg`, `.wss`) a `.lip` of the same name tells how far the speaker
//! opens their mouth over time. It is what `WAVToLIP` of the Arma 3 Tools makes of a `.wav`,
//! text with `\r\n` line ends:
//!
//! ```text
//! frame = 0.040
//! 0.000, 0
//! 0.200, 1
//! 0.280, 3
//! 1.360, -1
//! ```
//!
//! - `frame = ` and the length of a frame in seconds, always `0.040`.
//! - [`LipKey`]s, the time in seconds and how open the mouth is from then on, `0` (closed) to
//!   `7`. A key is only written when the value changes.
//! - The time the sound ends, with `-1`. Silent sounds have no keys and no end.
//!
//! [`Lip::from_pcm`] makes the same lip sync as `WAVToLIP` from 16 bit mono samples, byte for
//! byte on 4,300 random sounds at sample rates from 1 Hz to 200 kHz.
//!
//! Arma 3 has 116,081 `.lip` files (mostly radio protocol and mission dubbing), all of them read
//! and write back byte exact, 620 of them are silent. So do the 16,703 of about 2,150 mod
//! folders.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::real_virtuality::lip::Lip;
//!
//! let lip = Lip::from(&mut BufReader::new(File::open("CloseThatDoor.lip")?))?;
//! for key in &lip.keys {
//!     println!("{:.3} s: {}", key.time, key.value);
//! }
//! println!("{:?}", lip.value_at(0.5));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[allow(clippy::module_inception)]
mod lip;

pub use self::lip::*;
