//! Conversion between [`Rtm`] and [`Bmtr`], the way Binarize does it.

use half::f16;

use crate::errors::RtmError;

use super::{
    AnimMetadata, BMTR_ROTATION_SCALE, BMTR_VERSION, Bmtr, BmtrFrame, BmtrKeystone, BmtrTransform,
    KEYSTONE_STEP_SOUND, KEYSTONE_UNKNOWN, Rtm, RtmFrame, RtmKeystone, RtmMatrix, RtmMetadata,
    RtmMetadataValue, RtmName, RtmTransform, RtmVersion,
};

/// Binarize keeps `RTM_0101` animations of up to this many frames (and all `RTM_0100` ones)
/// loaded, see [`Bmtr::preload`].
const PRELOAD_MAX_FRAMES: usize = 3;

/// Signs of the 180 degree rotation around y, see [`Bmtr::flipped_xz`].
const FLIP: [f64; 3] = [-1.0, 1.0, -1.0];

impl BmtrTransform {
    /// The transform Binarize makes of an `RTM_0101` matrix, bit for bit: rotated by 180
    /// degrees around y if `flip_xz` (as Binarize always does), the rotation divided by the
    /// scale (root mean square of the rows) and quantized as quaternion, which is then
    /// scaled by the root of the scale and quantized again. A non-uniform scale or shear
    /// is not representable.
    #[must_use]
    // Plain f32 arithmetic in Binarize's order, fused multiply-adds would round differently.
    #[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
    pub fn from_matrix(matrix: &RtmMatrix, flip_xz: bool) -> Self {
        let sign = |i: usize| if flip_xz { FLIP[i] as f32 } else { 1.0 };
        let m: [[f32; 3]; 3] =
            std::array::from_fn(|i| std::array::from_fn(|j| matrix[i][j] * sign(i) * sign(j)));

        // Mean of the squared row lengths, summed in Binarize's order.
        let sq = |i: usize, j: usize| m[i][j] * m[i][j];
        let mut a = sq(2, 2) + sq(0, 2);
        let b = sq(0, 0) + sq(2, 0);
        let c = sq(1, 1) + sq(0, 1);
        let mut d = sq(1, 0) + sq(2, 1);
        a += sq(1, 2);
        d += c;
        a += b;
        a += d;
        let scale2 = (f64::from(a) * (1.0 / 3.0)) as f32;
        let inverse = 1.0 / scale2.sqrt();
        let unit = m.map(|row| row.map(|v| v * inverse));
        let rotation = quantized_quaternion(&unit);

        let factor = (inverse * scale2).sqrt();
        Self {
            rotation: rotation
                .map(|c| quantize(f32::from(c) * (1.0 / BMTR_ROTATION_SCALE) * factor)),
            translation: std::array::from_fn(|i| f16_binarize(matrix[3][i] * sign(i))),
        }
    }

    /// The `RTM_0101` matrix of this transform, undoing the rotation around y if
    /// `flipped_xz`. The length of the quaternion becomes the scale.
    #[must_use]
    #[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
    pub fn to_matrix(&self, flipped_xz: bool) -> RtmMatrix {
        let [x, y, z, w] = self.rotation().map(f64::from);
        let m = [
            [
                w * w + x * x - y * y - z * z,
                2.0 * (x * y - w * z),
                2.0 * (x * z + w * y),
            ],
            [
                2.0 * (x * y + w * z),
                w * w - x * x + y * y - z * z,
                2.0 * (y * z - w * x),
            ],
            [
                2.0 * (x * z - w * y),
                2.0 * (y * z + w * x),
                w * w - x * x - y * y + z * z,
            ],
        ];
        let sign = |i: usize| if flipped_xz { FLIP[i] } else { 1.0 };
        let translation = self.translation();
        let mut out = [[0.0; 3]; 4];
        for i in 0..3 {
            for j in 0..3 {
                out[i][j] = (m[i][j] * sign(i) * sign(j)) as f32;
            }
            out[3][i] = (f64::from(translation[i]) * sign(i)) as f32;
        }
        out
    }
}

/// Quantized quaternion (x, y, z, w) of a rotation matrix like Binarize, Shepperd's method
/// in `f32`: from the trace if it is positive, else from the largest diagonal element.
#[allow(clippy::many_single_char_names)] // The notation of the method.
fn quantized_quaternion(m: &[[f32; 3]; 3]) -> [i16; 4] {
    let trace = m[0][0] + m[1][1] + m[2][2];
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt();
        let t = 0.5 / s;
        let q = |v: f32| quantize(v * t);
        return [
            q(m[2][1] - m[1][2]),
            q(m[0][2] - m[2][0]),
            q(m[1][0] - m[0][1]),
            convert_i16(s * (0.5 * BMTR_ROTATION_SCALE)),
        ];
    }
    let mut i = usize::from(m[1][1] > m[0][0]);
    if m[2][2] > m[i][i] {
        i = 2;
    }
    let j = (i + 1) % 3;
    let k = (j + 1) % 3;
    let s = (m[i][i] - (m[k][k] + m[j][j]) + 1.0).sqrt();
    let t = if s == 0.0 { 0.0 } else { 0.5 / s };
    let mut q = [0.0f32; 4];
    q[i] = s * 0.5;
    q[3] = (m[k][j] - m[j][k]) * t;
    q[j] = (m[i][j] + m[j][i]) * t;
    q[k] = (m[i][k] + m[k][i]) * t;
    q.map(quantize)
}

/// `value` scaled by [`BMTR_ROTATION_SCALE`] and converted like Binarize.
fn quantize(value: f32) -> i16 {
    convert_i16(value * BMTR_ROTATION_SCALE)
}

/// `cvtss2si` and a 16 bit store: rounded half to even, wrapping, `0` for values past
/// `i32` and NaN.
fn convert_i16(value: f32) -> i16 {
    let rounded = value.round_ties_even();
    if rounded.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&rounded) {
        0
    } else {
        rounded as i32 as i16
    }
}

/// Binarize's half float conversion: below the smallest normal `+0`, mantissas truncated,
/// from `2^17` (and infinity and NaN) the sign and `0x7fff`.
pub(super) const fn f16_binarize(value: f32) -> f16 {
    let bits = value.to_bits();
    let exponent = ((bits >> 23) & 0xff) as i32 - 112;
    if exponent <= 0 {
        return f16::ZERO;
    }
    let sign = ((bits >> 16) & 0x8000) as u16;
    if exponent >= 32 {
        return f16::from_bits(sign | 0x7fff);
    }
    f16::from_bits(sign | ((exponent as u16) << 10) | ((bits & 0x7f_ffff) >> 13) as u16)
}

impl Bmtr {
    /// Binarizes an animation the way Binarize of the Arma 3 Tools does, with the same
    /// values: version 5, transforms flipped around y (see
    /// [`BmtrTransform::from_matrix`]), bone names in lower case, [`Bmtr::preload`] for up to
    /// 3 frames and for `RTM_0100`, metadata keys Binarize does not know dropped, keystones
    /// sorted by time with its sort (equal times end up in its order).
    ///
    /// With a `skeleton` (its bone names, ignoring case), bones not in it are dropped, the
    /// order stays the animation's. Without, all bones are kept. Binarize handles skeletons
    /// of up to 255 bones.
    #[must_use]
    pub fn from_rtm(rtm: &Rtm, skeleton: Option<&[&str]>) -> Self {
        let bones: Vec<(usize, String)> = rtm
            .bones
            .iter()
            .enumerate()
            .filter_map(|(index, bone)| {
                skeleton.map_or(Some(index), |skeleton| {
                    skeleton
                        .iter()
                        .any(|name| bone.matches(name))
                        .then_some(index)
                })
            })
            .map(|index| {
                (
                    index,
                    rtm.bones[index].to_string_lossy().to_ascii_lowercase(),
                )
            })
            .collect();

        let mut metadata: Vec<String> = Vec::new();
        let mut keystones = Vec::new();
        if let Some(source) = &rtm.metadata {
            for value in &source.values {
                if let Some(key) = AnimMetadata::from_name(&value.key) {
                    let index = key as usize;
                    if metadata.len() <= index {
                        metadata.resize(index + 1, String::new());
                    }
                    metadata[index].clone_from(&value.value);
                }
            }
            keystones = source
                .keystones
                .iter()
                .map(|k| BmtrKeystone {
                    kind: if k.name.eq_ignore_ascii_case("StepSound") {
                        KEYSTONE_STEP_SOUND
                    } else {
                        KEYSTONE_UNKNOWN
                    },
                    name: k.name.clone(),
                    time: k.time,
                    value: k.value.clone(),
                })
                .collect();
            sort_like_binarize(&mut keystones, |a, b| {
                let d = a.time - b.time;
                if d >= 0.0 { i32::from(d > 0.0) } else { -1 }
            });
        }

        Self {
            version: BMTR_VERSION,
            flipped_xz: true,
            step: rtm.step,
            preload: rtm.version == RtmVersion::Rtm0100 || rtm.frames.len() <= PRELOAD_MAX_FRAMES,
            bones: bones.iter().map(|(_, name)| name.clone()).collect(),
            metadata,
            keystones,
            frames: rtm
                .frames
                .iter()
                .map(|frame| BmtrFrame {
                    time: frame.time,
                    transforms: bones
                        .iter()
                        .map(|&(index, _)| {
                            frame
                                .transforms
                                .get(index)
                                .map_or(BmtrTransform::IDENTITY, |t| {
                                    BmtrTransform::from_matrix(&t.matrix, true)
                                })
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// The animation as `RTM_0101`, with an `RTM_MDAT` block if it has metadata or
    /// keystones. Fails for bone names of 32 bytes or more.
    pub fn to_rtm(&self) -> Result<Rtm, RtmError> {
        let bones = self
            .bones
            .iter()
            .map(|name| RtmName::new(name))
            .collect::<Result<Vec<_>, _>>()?;
        let metadata =
            (!self.metadata.is_empty() || !self.keystones.is_empty()).then(|| RtmMetadata {
                values: AnimMetadata::ALL
                    .into_iter()
                    .filter_map(|key| {
                        self.metadata(key).map(|value| RtmMetadataValue {
                            key: key.name().to_owned(),
                            value: value.to_owned(),
                        })
                    })
                    .collect(),
                keystones: self
                    .keystones
                    .iter()
                    .map(|k| RtmKeystone {
                        time: k.time,
                        name: k.name.clone(),
                        value: k.value.clone(),
                    })
                    .collect(),
            });
        Ok(Rtm {
            version: RtmVersion::Rtm0101,
            metadata,
            step: self.step,
            frames: self
                .frames
                .iter()
                .map(|frame| RtmFrame {
                    time: frame.time,
                    transforms: bones
                        .iter()
                        .zip(&frame.transforms)
                        .map(|(bone, t)| RtmTransform {
                            bone: *bone,
                            matrix: t.to_matrix(self.flipped_xz),
                        })
                        .collect(),
                })
                .collect(),
            bones,
        })
    }
}

/// The `qsort` of the Microsoft C runtime that Binarize uses: not stable, so equal elements
/// end up in its order. Below 9 elements it moves the largest to the end, else it swaps the
/// middle element to the front as pivot and partitions.
fn sort_like_binarize<T>(v: &mut [T], cmp: impl Fn(&T, &T) -> i32 + Copy) {
    const CUTOFF: usize = 8;
    if v.len() < 2 {
        return;
    }
    if v.len() <= CUTOFF {
        for hi in (1..v.len()).rev() {
            let mut max = 0;
            for p in 1..=hi {
                if cmp(&v[p], &v[max]) > 0 {
                    max = p;
                }
            }
            v.swap(max, hi);
        }
        return;
    }
    let hi = v.len() - 1;
    v.swap(v.len() / 2, 0);
    let mut lo_guy = 0;
    let mut hi_guy = hi + 1;
    loop {
        loop {
            lo_guy += 1;
            if lo_guy > hi || cmp(&v[lo_guy], &v[0]) > 0 {
                break;
            }
        }
        loop {
            hi_guy -= 1;
            if hi_guy == 0 || cmp(&v[hi_guy], &v[0]) < 0 {
                break;
            }
        }
        if hi_guy < lo_guy {
            break;
        }
        v.swap(lo_guy, hi_guy);
    }
    v.swap(0, hi_guy);
    let (low, rest) = v.split_at_mut(hi_guy);
    sort_like_binarize(low, cmp);
    sort_like_binarize(&mut rest[lo_guy - hi_guy..], cmp);
}
