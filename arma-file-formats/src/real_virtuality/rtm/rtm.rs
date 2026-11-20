use std::{
    borrow::Cow,
    io::{Read, Seek, Write},
};

use crate::errors::{AffError, RtmError};

use super::io::{
    read_array, read_bytes, read_exact, read_f32, read_u8, read_u32, read_vec3, write_f32s,
};

/// Signature of an unbinarized animation.
pub const RTM_SIGNATURE: &[u8; 8] = b"RTM_0101";
/// Signature of the older unbinarized format, with only the z of the step.
pub const RTM_0100_SIGNATURE: &[u8; 8] = b"RTM_0100";
/// Signature of the metadata block that can come before the animation.
pub const RTM_MDAT_SIGNATURE: &[u8; 8] = b"RTM_MDAT";

/// Bytes of a bone name in an `RTM_0101` file, including the NUL.
pub const RTM_NAME_SIZE: usize = 32;

/// A fixed size bone name of an `RTM_0101` file: up to 31 bytes, then a NUL.
///
/// The bytes after the NUL are kept as they are. The exporters leave whatever was in memory
/// there, and they differ between the header and each frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RtmName(pub [u8; RTM_NAME_SIZE]);

impl RtmName {
    /// A name padded with zeros. Fails for names of 32 bytes or more and names with a NUL.
    pub fn new(name: &str) -> Result<Self, RtmError> {
        if name.len() >= RTM_NAME_SIZE || name.contains('\0') {
            return Err(RtmError::InvalidBoneName(name.to_owned()));
        }
        let mut bytes = [0; RTM_NAME_SIZE];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Ok(Self(bytes))
    }

    /// The bytes up to the first NUL, all 32 if there is none.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        let len = self.0.iter().position(|&b| b == 0).unwrap_or(RTM_NAME_SIZE);
        &self.0[..len]
    }

    /// The name up to the first NUL.
    #[must_use]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self.as_bytes())
    }

    /// Whether the names before their NULs are equal, ignoring ASCII case like the engine.
    #[must_use]
    pub fn matches(&self, name: &str) -> bool {
        self.as_bytes().eq_ignore_ascii_case(name.as_bytes())
    }
}

impl Default for RtmName {
    fn default() -> Self {
        Self([0; RTM_NAME_SIZE])
    }
}

/// A 3x4 matrix of an `RTM_0101` file, four rows: the aside (x), up (y) and direction (z)
/// axes of the rotation, then the position.
pub type RtmMatrix = [[f32; 3]; 4];

/// The identity [`RtmMatrix`].
pub const RTM_IDENTITY: RtmMatrix = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, 0.0],
];

/// Which unbinarized format an [`Rtm`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RtmVersion {
    /// `RTM_0100`: the step is a single `f32`, its z. The binarizer logs "Old format used in
    /// animation" for these.
    Rtm0100,
    /// `RTM_0101`: the step has x, y and z.
    #[default]
    Rtm0101,
}

/// An unbinarized animation (`RTM_0101` or `RTM_0100`), as Oxygen, Object Builder and
/// `FBXToRTM` export it, see the [module docs](super).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Rtm {
    pub version: RtmVersion,
    /// The `RTM_MDAT` block before the animation, if there is one.
    pub metadata: Option<RtmMetadata>,
    /// Distance the animation moves the character over one cycle (x, y, z). Only z for
    /// [`RtmVersion::Rtm0100`].
    pub step: [f32; 3],
    /// The bone names of the header.
    pub bones: Vec<RtmName>,
    /// The frames, each with one transform per bone.
    pub frames: Vec<RtmFrame>,
}

/// One frame of an [`Rtm`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RtmFrame {
    /// Phase of the frame, `0` (start) to `1` (end).
    pub time: f32,
    /// One per bone of the header, in the same order.
    pub transforms: Vec<RtmTransform>,
}

/// The transform of a bone in one frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RtmTransform {
    /// The bone's name, the same as in the header up to the NUL.
    pub bone: RtmName,
    pub matrix: RtmMatrix,
}

/// The `RTM_MDAT` block: named values and keystones (animation events). Strings are a `u8`
/// length and the bytes, without NUL.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RtmMetadata {
    /// `u32` count, then key and value. The binarizer knows the keys of
    /// [`AnimMetadata`](super::AnimMetadata) and drops others.
    pub values: Vec<RtmMetadataValue>,
    /// `u32` count, then time, name and value.
    pub keystones: Vec<RtmKeystone>,
}

/// A named value of an [`RtmMetadata`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RtmMetadataValue {
    pub key: String,
    pub value: String,
}

/// An animation event of an [`RtmMetadata`], e.g. `StepSound` when a foot hits the ground.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RtmKeystone {
    /// Phase of the event, `0` to `1`.
    pub time: f32,
    pub name: String,
    pub value: String,
}

impl Rtm {
    /// Reads an unbinarized animation, with or without `RTM_MDAT` block.
    pub fn from<R: Read + Seek>(input: &mut R) -> Result<Self, AffError> {
        let signature: [u8; 8] = read_exact(input)?;
        Ok(Self::read_after_signature(signature, input)?)
    }

    /// Everything after the first signature.
    pub(super) fn read_after_signature<R: Read + Seek>(
        mut signature: [u8; 8],
        input: &mut R,
    ) -> Result<Self, RtmError> {
        let metadata = if &signature == RTM_MDAT_SIGNATURE {
            let metadata = RtmMetadata::read(input)?;
            signature = read_exact(input)?;
            Some(metadata)
        } else {
            None
        };
        let (version, step) = match &signature {
            RTM_SIGNATURE => (RtmVersion::Rtm0101, read_vec3(input)?),
            RTM_0100_SIGNATURE => (RtmVersion::Rtm0100, [0.0, 0.0, read_f32(input)?]),
            _ => return Err(RtmError::UnknownSignature(signature_lossy(&signature))),
        };
        let frame_count = read_u32(input)? as usize;
        let bone_count = read_u32(input)? as usize;
        let bones = read_array(input, bone_count, read_name)?;
        let frames = read_array(input, frame_count, |input| {
            Ok(RtmFrame {
                time: read_f32(input)?,
                transforms: read_array(input, bone_count, |input| {
                    Ok(RtmTransform {
                        bone: read_name(input)?,
                        matrix: [
                            read_vec3(input)?,
                            read_vec3(input)?,
                            read_vec3(input)?,
                            read_vec3(input)?,
                        ],
                    })
                })?,
            })
        })?;
        Ok(Self {
            version,
            metadata,
            step,
            bones,
            frames,
        })
    }

    /// Writes the animation. Every frame needs one transform per bone, an `RTM_0100` a step
    /// without x and y.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let frame_count = u32::try_from(self.frames.len()).map_err(|_| RtmError::TooLarge)?;
        let bone_count = u32::try_from(self.bones.len()).map_err(|_| RtmError::TooLarge)?;
        let mut data = Vec::with_capacity(
            28 + self.bones.len() * RTM_NAME_SIZE
                + self.frames.len() * (4 + self.bones.len() * (RTM_NAME_SIZE + 48)),
        );
        if let Some(metadata) = &self.metadata {
            data.extend_from_slice(RTM_MDAT_SIGNATURE);
            metadata.write(&mut data)?;
        }
        match self.version {
            RtmVersion::Rtm0101 => {
                data.extend_from_slice(RTM_SIGNATURE);
                write_f32s(&mut data, &self.step);
            }
            RtmVersion::Rtm0100 => {
                if self.step[..2] != [0.0, 0.0] {
                    return Err(RtmError::StepUnsupported.into());
                }
                data.extend_from_slice(RTM_0100_SIGNATURE);
                write_f32s(&mut data, &self.step[2..]);
            }
        }
        data.extend_from_slice(&frame_count.to_le_bytes());
        data.extend_from_slice(&bone_count.to_le_bytes());
        for bone in &self.bones {
            data.extend_from_slice(&bone.0);
        }
        for (index, frame) in self.frames.iter().enumerate() {
            if frame.transforms.len() != self.bones.len() {
                return Err(RtmError::TransformCountMismatch {
                    frame: index,
                    expected: self.bones.len(),
                    actual: frame.transforms.len(),
                }
                .into());
            }
            data.extend_from_slice(&frame.time.to_le_bytes());
            for transform in &frame.transforms {
                data.extend_from_slice(&transform.bone.0);
                for row in &transform.matrix {
                    write_f32s(&mut data, row);
                }
            }
        }
        out.write_all(&data)?;
        Ok(())
    }

    /// Index of the bone with this name, ignoring ASCII case.
    #[must_use]
    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.matches(name))
    }
}

impl RtmMetadata {
    fn read<R: Read>(input: &mut R) -> Result<Self, RtmError> {
        let count = read_u32(input)? as usize;
        let values = read_array(input, count, |input| {
            Ok(RtmMetadataValue {
                key: read_short_string(input)?,
                value: read_short_string(input)?,
            })
        })?;
        let count = read_u32(input)? as usize;
        let keystones = read_array(input, count, |input| {
            Ok(RtmKeystone {
                time: read_f32(input)?,
                name: read_short_string(input)?,
                value: read_short_string(input)?,
            })
        })?;
        Ok(Self { values, keystones })
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<(), RtmError> {
        let count = |len: usize| u32::try_from(len).map_err(|_| RtmError::TooLarge);
        out.extend_from_slice(&count(self.values.len())?.to_le_bytes());
        for value in &self.values {
            write_short_string(out, &value.key)?;
            write_short_string(out, &value.value)?;
        }
        out.extend_from_slice(&count(self.keystones.len())?.to_le_bytes());
        for keystone in &self.keystones {
            out.extend_from_slice(&keystone.time.to_le_bytes());
            write_short_string(out, &keystone.name)?;
            write_short_string(out, &keystone.value)?;
        }
        Ok(())
    }
}

fn read_name<R: Read>(input: &mut R) -> Result<RtmName, RtmError> {
    Ok(RtmName(read_exact(input)?))
}

/// A `u8` length and the bytes.
fn read_short_string<R: Read>(input: &mut R) -> Result<String, RtmError> {
    let len = read_u8(input)?;
    let bytes = read_bytes(input, usize::from(len))?;
    String::from_utf8(bytes).map_err(|e| RtmError::InvalidString(e.into_bytes()))
}

fn write_short_string(out: &mut Vec<u8>, value: &str) -> Result<(), RtmError> {
    let len = u8::try_from(value.len()).map_err(|_| RtmError::StringTooLong(value.to_owned()))?;
    out.push(len);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

pub(super) fn signature_lossy(signature: &[u8]) -> String {
    signature
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}
