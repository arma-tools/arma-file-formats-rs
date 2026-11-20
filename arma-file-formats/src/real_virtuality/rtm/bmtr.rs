use std::io::{Read, Seek, Write};

use half::f16;

use crate::errors::{AffError, RtmError};

use super::{
    convert::f16_binarize,
    io::{read_array, read_bytes, read_exact, read_f32, read_u8, read_u32, read_vec3, write_f32s},
    rtm::signature_lossy,
};

pub(super) const BMTR_SIGNATURE: &[u8; 4] = b"BMTR";

/// The version Binarize writes and every Arma 3 file has. Mods also have versions 3 (no
/// metadata and keystones) and 4 (no compression flags).
pub const BMTR_VERSION: u32 = 5;
/// The oldest version read.
pub const BMTR_MIN_VERSION: u32 = 3;

/// Bytes of a [`BmtrTransform`] in the file.
const TRANSFORM_SIZE: usize = 14;

/// Compression flag of an array stored as is.
const FLAG_RAW: u8 = 0;
/// Compression flag of an LZO1X compressed array.
const FLAG_LZO: u8 = 2;
/// Before version 5, arrays of this many bytes or more are compressed.
const LZO_MIN_SIZE: usize = 1024;
/// Binarize stores version 5 arrays of up to this many bytes as is.
const RAW_MAX_SIZE: usize = 256;
/// Binarize keeps the LZO data of a larger version 5 array if the array size divided by it
/// (as `f32`) is at least this, `1.0416667`.
const LZO_MIN_RATIO: f32 = f32::from_bits(0x3f85_5556);

/// `1.0` of a [`BmtrTransform::rotation`] component.
pub const BMTR_ROTATION_SCALE: f32 = 16384.0;

/// [`BmtrKeystone::kind`] of `StepSound`, the only keystone Binarize knows.
pub const KEYSTONE_STEP_SOUND: i32 = 0;
/// [`BmtrKeystone::kind`] of a keystone with a name Binarize does not know ("Invalid
/// animation keystone name").
pub const KEYSTONE_UNKNOWN: i32 = -1;

/// The keys of the metadata values, [`Bmtr::metadata`] is indexed by them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimMetadata {
    WalkCycles = 0,
    AnimLength = 1,
}

impl AnimMetadata {
    pub const ALL: [Self; 2] = [Self::WalkCycles, Self::AnimLength];

    /// The key in an `RTM_MDAT` block.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::WalkCycles => "WalkCycles",
            Self::AnimLength => "AnimLength",
        }
    }

    /// The key with this name, ignoring ASCII case like Binarize.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|key| key.name().eq_ignore_ascii_case(name))
    }
}

/// A binarized animation (`BMTR`), as the game ships them, see the [module docs](super).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bmtr {
    /// [`BMTR_MIN_VERSION`] to [`BMTR_VERSION`].
    pub version: u32,
    /// The byte after the version, set by Binarize in every file: the transforms are
    /// rotated by 180 degrees around y (x and z negated) from the `RTM_0101` ones.
    pub flipped_xz: bool,
    /// Distance the animation moves the character over one cycle (x, y, z).
    pub step: [f32; 3],
    /// The `u32` after the frame count. Binarize sets it for animations of up to 3 frames
    /// and for all `RTM_0100` sources, the engine then prepares all frames when it loads the
    /// file.
    pub preload: bool,
    /// Bone names, lower case from Binarize. It keeps the bones of the skeleton, in the order
    /// of the source.
    pub bones: Vec<String>,
    /// Metadata values indexed by [`AnimMetadata`], empty for keys not set. Binarize writes
    /// up to the last key set. Not in version 3.
    pub metadata: Vec<String>,
    /// Animation events, sorted by time. Not in version 3.
    pub keystones: Vec<BmtrKeystone>,
    /// The frames, each with one transform per bone.
    pub frames: Vec<BmtrFrame>,
}

/// An animation event of a [`Bmtr`], e.g. `StepSound` at the phases a foot hits the ground.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BmtrKeystone {
    /// [`KEYSTONE_STEP_SOUND`] or [`KEYSTONE_UNKNOWN`].
    pub kind: i32,
    pub name: String,
    /// Phase of the event, `0` to `1`.
    pub time: f32,
    /// The engine reads it as a float. Empty in every game file.
    pub value: String,
}

/// One frame of a [`Bmtr`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BmtrFrame {
    /// Phase of the frame, `0` (start) to `1` (end).
    pub time: f32,
    /// One per bone, in the order of [`Bmtr::bones`].
    pub transforms: Vec<BmtrTransform>,
}

/// The transform of a bone in one frame: a quantized quaternion and a half float translation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BmtrTransform {
    /// Quaternion x, y, z, w, scaled by [`BMTR_ROTATION_SCALE`]. The identity is
    /// `[0, 0, 0, 16384]`. A uniform scale `s` of the bone gives a length of `sqrt(s)`.
    pub rotation: [i16; 4],
    /// Translation x, y, z.
    pub translation: [f16; 3],
}

impl BmtrTransform {
    /// No rotation and no translation.
    pub const IDENTITY: Self = Self {
        rotation: [0, 0, 0, 16384],
        translation: [f16::ZERO; 3],
    };

    /// Quantizes a quaternion (x, y, z, w) and a translation. Rotation components are
    /// rounded to the nearest step and saturate at `±32767 / 16384`. Translations are
    /// converted like Binarize: below `2^-14` to `+0`, else truncated to the half float
    /// towards zero, from `2^17` on to `±0x7fff` (a NaN).
    #[must_use]
    pub fn new(rotation: [f32; 4], translation: [f32; 3]) -> Self {
        Self {
            rotation: rotation
                .map(|c| (c * BMTR_ROTATION_SCALE).round().clamp(-32768.0, 32767.0) as i16),
            translation: translation.map(f16_binarize),
        }
    }

    /// The quaternion (x, y, z, w), not normalized.
    #[must_use]
    pub fn rotation(&self) -> [f32; 4] {
        self.rotation.map(|c| f32::from(c) / BMTR_ROTATION_SCALE)
    }

    #[must_use]
    pub fn translation(&self) -> [f32; 3] {
        self.translation.map(f16::to_f32)
    }

    fn parse(data: &[u8; TRANSFORM_SIZE]) -> Self {
        let i16_at = |i: usize| i16::from_le_bytes([data[2 * i], data[2 * i + 1]]);
        let f16_at = |i: usize| f16::from_le_bytes([data[2 * i], data[2 * i + 1]]);
        Self {
            rotation: [i16_at(0), i16_at(1), i16_at(2), i16_at(3)],
            translation: [f16_at(4), f16_at(5), f16_at(6)],
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        for c in self.rotation {
            out.extend_from_slice(&c.to_le_bytes());
        }
        for c in self.translation {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
}

impl Bmtr {
    /// Reads a `BMTR` animation.
    pub fn from<R: Read + Seek>(input: &mut R) -> Result<Self, AffError> {
        let signature: [u8; 4] = read_exact(input)?;
        if &signature != BMTR_SIGNATURE {
            return Err(RtmError::UnknownSignature(signature_lossy(&signature)).into());
        }
        Ok(Self::read_body(input)?)
    }

    /// Everything after the signature.
    pub(super) fn read_body<R: Read + Seek>(input: &mut R) -> Result<Self, RtmError> {
        let version = read_u32(input)?;
        if !(BMTR_MIN_VERSION..=BMTR_VERSION).contains(&version) {
            return Err(RtmError::UnsupportedVersion(version));
        }
        let flipped_xz = read_u8(input)? != 0;
        let step = read_vec3(input)?;
        let frame_count = read_u32(input)? as usize;
        let preload = read_u32(input)? != 0;
        let bone_count = read_u32(input)? as usize;
        let name_count = read_u32(input)? as usize;
        // Without frames Binarize writes 0 bones but still the names.
        if name_count != bone_count && !(bone_count == 0 && frame_count == 0) {
            return Err(RtmError::BoneCountMismatch {
                header: bone_count,
                names: name_count,
            });
        }
        let bones = read_array(input, name_count, read_string)?;
        let (metadata, keystones) = if version >= 4 {
            let count = read_u32(input)? as usize;
            let metadata = read_array(input, count, read_string)?;
            let count = read_u32(input)? as usize;
            let keystones = read_array(input, count, |input| {
                Ok(BmtrKeystone {
                    kind: read_u32(input)? as i32,
                    name: read_string(input)?,
                    time: read_f32(input)?,
                    value: read_string(input)?,
                })
            })?;
            (metadata, keystones)
        } else {
            (Vec::new(), Vec::new())
        };

        let times = read_compressed(input, version, 4)?;
        if times.len() / 4 != frame_count {
            return Err(RtmError::FrameCountMismatch {
                header: frame_count,
                times: times.len() / 4,
            });
        }
        let mut index = 0;
        let frames = read_array(input, frame_count, |input| {
            index += 1;
            let index = index - 1;
            let data = read_compressed(input, version, TRANSFORM_SIZE)?;
            let count = data.len() / TRANSFORM_SIZE;
            if count != bone_count {
                return Err(RtmError::TransformCountMismatch {
                    frame: index,
                    expected: bone_count,
                    actual: count,
                });
            }
            Ok(data
                .as_chunks::<TRANSFORM_SIZE>()
                .0
                .iter()
                .map(BmtrTransform::parse)
                .collect::<Vec<_>>())
        })?
        .into_iter()
        .zip(times.as_chunks::<4>().0)
        .map(|(transforms, time)| BmtrFrame {
            time: f32::from_le_bytes(*time),
            transforms,
        })
        .collect();

        Ok(Self {
            version,
            flipped_xz,
            step,
            preload,
            bones,
            metadata,
            keystones,
            frames,
        })
    }

    /// Writes the animation in its [`Bmtr::version`]. Version 5 arrays are compressed like
    /// Binarize does it, giving the bytes of the game's files. Version 3 has no metadata and
    /// keystones.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let version = self.version;
        if !(BMTR_MIN_VERSION..=BMTR_VERSION).contains(&version) {
            return Err(RtmError::UnsupportedVersion(version).into());
        }
        if version < 4 && (!self.metadata.is_empty() || !self.keystones.is_empty()) {
            return Err(RtmError::MetadataUnsupported.into());
        }
        let count = |len: usize| u32::try_from(len).map_err(|_| RtmError::TooLarge);
        let mut data = Vec::new();
        data.extend_from_slice(BMTR_SIGNATURE);
        data.extend_from_slice(&version.to_le_bytes());
        data.push(u8::from(self.flipped_xz));
        write_f32s(&mut data, &self.step);
        data.extend_from_slice(&count(self.frames.len())?.to_le_bytes());
        data.extend_from_slice(&u32::from(self.preload).to_le_bytes());
        let bone_count = if self.frames.is_empty() {
            0
        } else {
            self.bones.len()
        };
        data.extend_from_slice(&count(bone_count)?.to_le_bytes());
        data.extend_from_slice(&count(self.bones.len())?.to_le_bytes());
        for bone in &self.bones {
            write_string(&mut data, bone)?;
        }
        if version >= 4 {
            data.extend_from_slice(&count(self.metadata.len())?.to_le_bytes());
            for value in &self.metadata {
                write_string(&mut data, value)?;
            }
            data.extend_from_slice(&count(self.keystones.len())?.to_le_bytes());
            for keystone in &self.keystones {
                data.extend_from_slice(&keystone.kind.to_le_bytes());
                write_string(&mut data, &keystone.name)?;
                data.extend_from_slice(&keystone.time.to_le_bytes());
                write_string(&mut data, &keystone.value)?;
            }
        }

        let mut times = Vec::with_capacity(self.frames.len() * 4);
        for frame in &self.frames {
            times.extend_from_slice(&frame.time.to_le_bytes());
        }
        write_compressed(&mut data, version, self.frames.len(), &times)?;
        let mut transforms = Vec::with_capacity(self.bones.len() * TRANSFORM_SIZE);
        for (index, frame) in self.frames.iter().enumerate() {
            if frame.transforms.len() != self.bones.len() {
                return Err(RtmError::TransformCountMismatch {
                    frame: index,
                    expected: self.bones.len(),
                    actual: frame.transforms.len(),
                }
                .into());
            }
            transforms.clear();
            for transform in &frame.transforms {
                transform.write(&mut transforms);
            }
            write_compressed(&mut data, version, frame.transforms.len(), &transforms)?;
        }
        out.write_all(&data)?;
        Ok(())
    }

    /// Index of the bone with this name, ignoring ASCII case like the engine.
    #[must_use]
    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.eq_ignore_ascii_case(name))
    }

    /// The metadata value of this key, `None` if not set.
    #[must_use]
    pub fn metadata(&self, key: AnimMetadata) -> Option<&str> {
        self.metadata
            .get(key as usize)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }
}

/// A `u32` element count, then, unless it is 0, the elements. Version 5 has a compression
/// flag before them, older versions compress arrays of [`LZO_MIN_SIZE`] bytes or more.
fn read_compressed<R: Read + Seek>(
    input: &mut R,
    version: u32,
    element_size: usize,
) -> Result<Vec<u8>, RtmError> {
    let count = read_u32(input)? as usize;
    let size = count.checked_mul(element_size).ok_or(RtmError::Truncated)?;
    if size == 0 {
        return Ok(Vec::new());
    }
    let compressed = if version >= 5 {
        match read_u8(input)? {
            FLAG_RAW => false,
            FLAG_LZO => true,
            flag => return Err(RtmError::UnknownCompression(flag)),
        }
    } else {
        size >= LZO_MIN_SIZE
    };
    if !compressed {
        return read_bytes(input, size);
    }
    let data =
        lzokay_native::decompress_bounded(input, size).map_err(|e| RtmError::Lzo(e.to_string()))?;
    if data.len() == size {
        Ok(data)
    } else {
        Err(RtmError::Lzo(format!(
            "decompressed to {} bytes, expected {size}",
            data.len()
        )))
    }
}

/// Version 5 compresses like Binarize: arrays above [`RAW_MAX_SIZE`] bytes with its
/// compressor, kept if that reaches [`LZO_MIN_RATIO`]. Older versions have to compress the
/// arrays of [`LZO_MIN_SIZE`] bytes or more, with lzokay-native.
fn write_compressed(
    out: &mut Vec<u8>,
    version: u32,
    count: usize,
    data: &[u8],
) -> Result<(), RtmError> {
    out.extend_from_slice(
        &u32::try_from(count)
            .map_err(|_| RtmError::TooLarge)?
            .to_le_bytes(),
    );
    if data.is_empty() {
        return Ok(());
    }
    let lzo = || lzokay_native::compress(data).map_err(|e| RtmError::Lzo(e.to_string()));
    if version < 5 {
        if data.len() >= LZO_MIN_SIZE {
            out.extend_from_slice(&lzo()?);
        } else {
            out.extend_from_slice(data);
        }
        return Ok(());
    }
    let compressed = (data.len() > RAW_MAX_SIZE).then(|| super::lzo::compress(data));
    match compressed {
        Some(compressed) if data.len() as f32 / compressed.len() as f32 >= LZO_MIN_RATIO => {
            out.push(FLAG_LZO);
            out.extend_from_slice(&compressed);
        }
        _ => {
            out.push(FLAG_RAW);
            out.extend_from_slice(data);
        }
    }
    Ok(())
}

/// A NUL terminated string.
fn read_string<R: Read>(input: &mut R) -> Result<String, RtmError> {
    let mut bytes = Vec::new();
    loop {
        match read_u8(input)? {
            0 => break,
            b => bytes.push(b),
        }
    }
    String::from_utf8(bytes).map_err(|e| RtmError::InvalidString(e.into_bytes()))
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), RtmError> {
    if value.contains('\0') {
        return Err(RtmError::InvalidString(value.as_bytes().to_vec()));
    }
    out.extend_from_slice(value.as_bytes());
    out.push(0);
    Ok(())
}
