//! Enfusion Opus sound (`.snd`).
//!
//! The game's music: Workbench's Audio Editor converts 16 bit PCM `.wav` files to it ("Convert
//! Audio Files", target format `OPUS`), upsampling 44.1 kHz to 48 kHz. A `.snd` is a RIFF file
//! of form type `OPUS`, little endian, with these chunks in this order:
//!
//! - `VERS`: a size of `1`, the version byte (`5`) and 3 zero bytes, see [`SndVersion`].
//! - `fmt `: the PCM format the packets decode to, a `WAVEFORMAT` like in `.wav` files, see
//!   [`SndFormat`].
//! - `ENV `: the loudness of 32 equal parts of the sound and the "Dynamic Volume" option, see
//!   [`SndEnvelope`]. Optional for the engine, one game file has none.
//! - `opus`: `u32` packet count and `u32` pre-skip, see [`Snd::packet_count`] and
//!   [`Snd::pre_skip`].
//! - `data`: its size is the size of the decoded PCM ([`Snd::data_size`]), not of its content.
//!   The content is the rest of the file: Opus packets, each a `u16` length and the packet.
//!
//! The RIFF size is the header size plus [`Snd::data_size`], as if the `data` chunk held the
//! PCM.
//!
//! Workbench encodes 40 ms packets (1920 samples at 48 kHz) with libopus, after padding the
//! PCM with silence to a multiple of 40 ms, and one more packet of silence to flush the
//! encoder. [`Snd::write_ogg`] puts the packets in an Ogg Opus file that common players and
//! decoders take.
//!
//! ```no_run
//! use std::{fs::File, io::BufReader};
//!
//! use arma_file_formats::enfusion::snd::Snd;
//!
//! let snd = Snd::from(&mut BufReader::new(File::open("MU_Ascent.snd")?))?;
//! println!("{} s", snd.duration().as_secs_f32());
//! snd.write_ogg(&mut File::create("MU_Ascent.opus")?, 0)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod ogg;
#[allow(clippy::module_inception)]
mod snd;

pub use self::{ogg::packet_samples, snd::*};
