//! Real Virtuality sounds (`.wss`).
//!
//! What `WAVToWSS` of the Arma 3 Tools makes of 16 bit PCM `.wav` files and the format of most
//! of Arma 3's sounds. A `.wss` is a 26 byte header and the sample data, little endian:
//!
//! - `WSS0`.
//! - `u32` compression, see [`WssCompression`]: `0` PCM as in the `.wav`, `8` an 8 bit and `4`
//!   a 4 bit "deltapack".
//! - A `WAVEFORMAT` with `wBitsPerSample` (16 bytes), see [`WssFormat`]: the PCM the data
//!   decodes to.
//! - A `u16` where a `WAVEFORMATEX` has its `cbSize`, see [`Wss::extra`].
//! - The data, interleaved by channel.
//!
//! The deltapacks store one byte or nibble per sample: the index of a step that is added to
//! the previous sample of the channel. The steps of the 8 bit deltapack grow exponentially up to
//! `±32768`, see [`DELTA8_STEPS`], the 4 bit deltapack (`-d4`, "voice") has 15 steps up to
//! `±8192`, see [`DELTA4_STEPS`]. [`Wss::from_pcm`] encodes like `WAVToWSS`, [`Wss::decode`]
//! and [`Wss::write_wav`] decode like its `-convert`.
//!
//! Arma 3 has 11,942 `.wss` files: 11,938 8 bit deltapacks (mono and stereo, mostly
//! 44.1 kHz) and 4 PCM files (8, 16 and 24 bit). All of them decode to the same samples as with
//! `WAVToWSS -convert` and round trip byte exact, and [`Wss::from_pcm`] encodes their samples
//! back to the same bytes. About 2,150 mod folders add 9,470 8 bit and 20 4 bit deltapacks that
//! decode like `-convert` as well. Most of them come from other encoders and don't encode back
//! to the same bytes.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::real_virtuality::wss::Wss;
//!
//! let wss = Wss::from(&mut BufReader::new(File::open("notificationDefault.wss")?))?;
//! println!("{} channels, {} s", wss.format.channels, wss.duration().as_secs_f32());
//! wss.write_wav(&mut File::create("notificationDefault.wav")?)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod delta;
#[allow(clippy::module_inception)]
mod wss;

pub use self::{delta::*, wss::*};
