//! Editable models (MLOD `.p3d`), what Object Builder saves and Binarize makes ODOL from.
//!
//! Little endian, `MLOD`, the version `0x101` as `u32` and the `u32` number of LODs, then each
//! [`MlodLod`] to the end of the file:
//!
//! - `P3DM`, the version `0x1c` and `0x100` as `u32`, the `u32` numbers of points, normals and
//!   faces and `u32` [`MlodLod::flags`].
//! - The [`MlodPoint`]s: position as 3 `f32`, `u32` flags.
//! - The normals as 3 `f32`.
//! - The [`MlodFace`]s: the `u32` number of sides (3 or 4), always 4 [`MlodVertex`]es (`u32`
//!   point, `u32` normal, `f32` u and v), `u32` flags, texture and material as `NUL` terminated
//!   strings.
//! - `TAGG` and the [`MlodTagg`]s: a `u8` (`1`), the `NUL` terminated name, the `u32` size and
//!   the data. Names starting with `#` are special, the others are named selections. The last
//!   one is `#EndOfFile#` without data. Like Object Builder, the size of `#UVSet#` is ignored:
//!   its data is always the `u32` id and a u and v `f32` per used vertex of each face.
//! - The [`MlodLod::resolution`] as `f32`.
//!
//! The game itself ships ODOL only. The 7 MLODs of the Arma 3 Tools and 451 of the 455 in mods
//! read and write byte exact. The other 4 store a `#UVSet#` size 4 too small, they are written
//! back with the right size.
//!
//! ```no_run
//! use arma_file_formats::real_virtuality::p3d::Mlod;
//!
//! let mlod = Mlod::from_path("model.p3d")?;
//! for lod in &mlod.lods {
//!     println!("{}: {} points, {} faces", lod.resolution, lod.points.len(), lod.faces.len());
//!     for selection in lod.named_selections() {
//!         println!("  {}", selection.name);
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    io::{Read, Write},
    path::Path,
};

use crate::errors::{AffError, MlodError};

/// The signature of an MLOD.
pub const MLOD_SIGNATURE: &[u8; 4] = b"MLOD";
/// The only version of [`MLOD_SIGNATURE`].
pub const MLOD_VERSION: u32 = 0x101;
/// The signature of an [`MlodLod`].
pub const P3DM_SIGNATURE: &[u8; 4] = b"P3DM";
/// The major version of [`P3DM_SIGNATURE`].
pub const P3DM_MAJOR_VERSION: u32 = 0x1c;
/// The minor version of [`P3DM_SIGNATURE`].
pub const P3DM_MINOR_VERSION: u32 = 0x100;

const TAGG_SIGNATURE: &[u8; 4] = b"TAGG";
const END_OF_FILE: &str = "#EndOfFile#";
const SHARP_EDGES: &str = "#SharpEdges#";
const PROPERTY: &str = "#Property#";
const MASS: &str = "#Mass#";
const UV_SET: &str = "#UVSet#";
const ANIMATION: &str = "#Animation#";
const SELECTED: &str = "#Selected#";
const HIDDEN: &str = "#Hide#";
const LOCK: &str = "#Lock#";
/// Property names and values are this long, padded with `NUL`.
const PROPERTY_LEN: usize = 64;

/// An editable model, see the [module docs](self).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Mlod {
    pub lods: Vec<MlodLod>,
}

/// A level of detail.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MlodLod {
    /// The LOD's resolution, like `1.0` for the first visual LOD and `1e13` for the geometry,
    /// the same values as [`super::Resolution::value`].
    pub resolution: f32,
    /// `0` in every known model.
    pub flags: u32,
    pub points: Vec<MlodPoint>,
    pub normals: Vec<[f32; 3]>,
    pub faces: Vec<MlodFace>,
    /// In file order without the closing `#EndOfFile#`.
    pub taggs: Vec<MlodTagg>,
}

/// A point of an [`MlodLod`].
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MlodPoint {
    pub position: [f32; 3],
    /// Lighting, fog, land contact and so on, the `POINT_*` constants of Object Builder's
    /// `O2Scripts\std\flags.inc`. `0x0100_0000` is hidden.
    pub flags: u32,
}

/// A triangle or quad of an [`MlodLod`].
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MlodFace {
    /// `3` or `4`, how many of [`MlodFace::vertices`] are used, see [`MlodFace::used`].
    pub sides: u32,
    /// The file always has 4, the last is unused in triangles.
    pub vertices: [MlodVertex; 4],
    /// Lighting, shadows, z bias and so on, the `FACE_*` constants of Object Builder's
    /// `O2Scripts\std\flags.inc`.
    pub flags: u32,
    /// Path of the texture, can be empty.
    pub texture: String,
    /// Path of the `.rvmat`, can be empty.
    pub material: String,
}

/// A corner of an [`MlodFace`].
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MlodVertex {
    /// Index into [`MlodLod::points`].
    pub point: u32,
    /// Index into [`MlodLod::normals`].
    pub normal: u32,
    /// Texture coordinates of the first UV set.
    pub u: f32,
    pub v: f32,
}

/// The extra data of an [`MlodLod`].
#[derive(Debug, Clone, PartialEq)]
pub enum MlodTagg {
    /// `#SharpEdges#`: pairs of [`MlodLod::points`] indices.
    SharpEdges(Vec<[u32; 2]>),
    /// `#Property#`: a named property like `class` = `house`. Both are at most 63 bytes.
    Property { name: String, value: String },
    /// `#Mass#`: the mass of each point (geometry LODs).
    Mass(Vec<f32>),
    /// `#UVSet#`: texture coordinates of each used vertex of each face, in face order. Set `0`
    /// repeats the coordinates of the [`MlodVertex`]es.
    UvSet { id: u32, uvs: Vec<[f32; 2]> },
    /// `#Animation#`: a frame of the point positions at a time (or phase).
    Animation { time: f32, positions: Vec<[f32; 3]> },
    /// `#Selected#`: what is selected in Object Builder.
    Selected(MlodSelection),
    /// `#Hide#`: what is hidden in Object Builder.
    Hidden(MlodSelection),
    /// `#Lock#`: what is locked in Object Builder.
    Lock(MlodSelection),
    /// A named selection (the name doesn't start with `#`).
    NamedSelection {
        name: String,
        selection: MlodSelection,
    },
    /// Any other tagg, also the known ones with another first byte than `1`.
    Unknown {
        active: u8,
        name: String,
        data: Vec<u8>,
    },
}

/// Which points and faces of an [`MlodLod`] a selection has, one byte each.
///
/// A point byte is `0` for not selected, otherwise the weight `(256 - byte) / 255`: `1` is
/// fully selected, `255` the smallest weight, see [`MlodSelection::weight`]. Faces are `0` or `1`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MlodSelection {
    pub points: Vec<u8>,
    pub faces: Vec<u8>,
}

/// A named selection of an [`MlodLod`], see [`MlodLod::named_selections`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MlodNamedSelection<'a> {
    pub name: &'a str,
    pub selection: &'a MlodSelection,
}

impl Mlod {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, AffError> {
        Ok(Self::parse(&std::fs::read(path)?)?)
    }

    pub fn from<I: Read>(input: &mut I) -> Result<Self, AffError> {
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        Ok(Self::parse(&bytes)?)
    }

    fn parse(bytes: &[u8]) -> Result<Self, MlodError> {
        let mut input = Reader { bytes, pos: 0 };
        let signature = input.take(4)?;
        if signature != MLOD_SIGNATURE {
            return Err(MlodError::UnknownSignature(
                String::from_utf8_lossy(signature).into_owned(),
            ));
        }
        let version = input.u32()?;
        if version != MLOD_VERSION {
            return Err(MlodError::UnsupportedVersion(version));
        }
        let count = input.u32()? as usize;
        // The smallest LOD has 49 bytes.
        let mut lods = Vec::with_capacity(count.min(input.remaining() / 49));
        for _ in 0..count {
            lods.push(MlodLod::parse(&mut input)?);
        }
        if input.remaining() != 0 {
            return Err(MlodError::TrailingData(input.remaining()));
        }
        Ok(Self { lods })
    }

    /// Writes the model at the current position of `out`.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let mut bytes = Vec::new();
        bytes.extend(MLOD_SIGNATURE);
        bytes.extend(MLOD_VERSION.to_le_bytes());
        put_len(&mut bytes, self.lods.len())?;
        for lod in &self.lods {
            lod.write(&mut bytes)?;
        }
        out.write_all(&bytes)?;
        Ok(())
    }

    /// The first LOD of a resolution.
    #[must_use]
    pub fn lod(&self, resolution: f32) -> Option<&MlodLod> {
        self.lods.iter().find(|lod| lod.resolution == resolution)
    }
}

impl MlodLod {
    fn parse(input: &mut Reader) -> Result<Self, MlodError> {
        let signature = input.take(4)?;
        if signature != P3DM_SIGNATURE {
            return Err(MlodError::UnknownLodSignature(
                String::from_utf8_lossy(signature).into_owned(),
            ));
        }
        let major = input.u32()?;
        let minor = input.u32()?;
        if (major, minor) != (P3DM_MAJOR_VERSION, P3DM_MINOR_VERSION) {
            return Err(MlodError::UnsupportedLodVersion { major, minor });
        }
        let point_count = input.u32()? as usize;
        let normal_count = input.u32()? as usize;
        let face_count = input.u32()? as usize;
        let flags = input.u32()?;

        let mut points = Vec::with_capacity(point_count.min(input.remaining() / 16));
        for _ in 0..point_count {
            points.push(MlodPoint {
                position: input.vec3()?,
                flags: input.u32()?,
            });
        }
        let mut normals = Vec::with_capacity(normal_count.min(input.remaining() / 12));
        for _ in 0..normal_count {
            normals.push(input.vec3()?);
        }
        // Without strings a face has 74 bytes.
        let mut faces = Vec::with_capacity(face_count.min(input.remaining() / 74));
        for _ in 0..face_count {
            faces.push(MlodFace::parse(input)?);
        }

        if input.take(4)? != TAGG_SIGNATURE {
            return Err(MlodError::TaggsMissing);
        }
        let mut lod = Self {
            resolution: 0.0,
            flags,
            points,
            normals,
            faces,
            taggs: Vec::new(),
        };
        // Object Builder ignores the size of UV sets, a few mod models store 4 less (without
        // the id).
        let uv_set_size = 4 + 8 * lod.corners();
        loop {
            let active = input.u8()?;
            let name = input.string()?;
            let size = input.u32()? as usize;
            let size = if active == 1 && name == UV_SET {
                uv_set_size
            } else {
                size
            };
            let data = input.take(size)?;
            if active == 1 && name == END_OF_FILE && data.is_empty() {
                break;
            }
            let tagg = lod.parse_tagg(active, name, data)?;
            lod.taggs.push(tagg);
        }
        lod.resolution = input.f32()?;
        Ok(lod)
    }

    fn parse_tagg(&self, active: u8, name: String, data: &[u8]) -> Result<MlodTagg, MlodError> {
        if active != 1 {
            return Ok(MlodTagg::Unknown {
                active,
                name,
                data: data.to_vec(),
            });
        }
        let points = self.points.len();
        let faces = self.faces.len();
        let check = |expected: usize| {
            if data.len() == expected {
                Ok(())
            } else {
                Err(MlodError::InvalidTaggSize {
                    name: name.clone(),
                    size: data.len(),
                    expected,
                })
            }
        };
        let selection = || {
            check(points + faces)?;
            Ok::<_, MlodError>(MlodSelection {
                points: data[..points].to_vec(),
                faces: data[points..].to_vec(),
            })
        };
        let mut input = Reader {
            bytes: data,
            pos: 0,
        };
        Ok(match name.as_str() {
            SHARP_EDGES => {
                check(data.len() / 8 * 8)?;
                let mut edges = Vec::with_capacity(data.len() / 8);
                while input.remaining() != 0 {
                    edges.push([input.u32()?, input.u32()?]);
                }
                MlodTagg::SharpEdges(edges)
            }
            PROPERTY => {
                check(2 * PROPERTY_LEN)?;
                let name = property_string(&data[..PROPERTY_LEN])?;
                let value = property_string(&data[PROPERTY_LEN..])?;
                let same = |value: &str, bytes: &[u8]| {
                    property_bytes(value).is_ok_and(|written| written == bytes)
                };
                if !same(&name, &data[..PROPERTY_LEN]) || !same(&value, &data[PROPERTY_LEN..]) {
                    // Bytes after the NUL or no NUL, which wouldn't be written back.
                    return Ok(MlodTagg::Unknown {
                        active,
                        name: PROPERTY.to_owned(),
                        data: data.to_vec(),
                    });
                }
                MlodTagg::Property { name, value }
            }
            MASS => {
                check(4 * points)?;
                MlodTagg::Mass((0..points).map(|_| input.f32()).collect::<Result<_, _>>()?)
            }
            UV_SET => {
                let corners = self.corners();
                check(4 + 8 * corners)?;
                let id = input.u32()?;
                let mut uvs = Vec::with_capacity(corners);
                for _ in 0..corners {
                    uvs.push([input.f32()?, input.f32()?]);
                }
                MlodTagg::UvSet { id, uvs }
            }
            ANIMATION => {
                check(4 + 12 * points)?;
                let time = input.f32()?;
                let positions = (0..points)
                    .map(|_| input.vec3())
                    .collect::<Result<_, _>>()?;
                MlodTagg::Animation { time, positions }
            }
            SELECTED => MlodTagg::Selected(selection()?),
            HIDDEN => MlodTagg::Hidden(selection()?),
            LOCK => MlodTagg::Lock(selection()?),
            _ if !name.is_empty() && !name.starts_with('#') => MlodTagg::NamedSelection {
                selection: selection()?,
                name,
            },
            _ => MlodTagg::Unknown {
                active,
                name,
                data: data.to_vec(),
            },
        })
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<(), MlodError> {
        out.extend(P3DM_SIGNATURE);
        out.extend(P3DM_MAJOR_VERSION.to_le_bytes());
        out.extend(P3DM_MINOR_VERSION.to_le_bytes());
        put_len(out, self.points.len())?;
        put_len(out, self.normals.len())?;
        put_len(out, self.faces.len())?;
        out.extend(self.flags.to_le_bytes());
        for point in &self.points {
            put_vec3(out, point.position);
            out.extend(point.flags.to_le_bytes());
        }
        for normal in &self.normals {
            put_vec3(out, *normal);
        }
        for face in &self.faces {
            out.extend(face.sides.to_le_bytes());
            for vertex in &face.vertices {
                out.extend(vertex.point.to_le_bytes());
                out.extend(vertex.normal.to_le_bytes());
                out.extend(vertex.u.to_le_bytes());
                out.extend(vertex.v.to_le_bytes());
            }
            out.extend(face.flags.to_le_bytes());
            put_string(out, &face.texture)?;
            put_string(out, &face.material)?;
        }
        out.extend(TAGG_SIGNATURE);
        for tagg in &self.taggs {
            self.write_tagg(out, tagg)?;
        }
        put_tagg(out, 1, END_OF_FILE, &[])?;
        out.extend(self.resolution.to_le_bytes());
        Ok(())
    }

    fn write_tagg(&self, out: &mut Vec<u8>, tagg: &MlodTagg) -> Result<(), MlodError> {
        let mut data = Vec::new();
        let selection = |data: &mut Vec<u8>, name: &str, selection: &MlodSelection| {
            if selection.points.len() != self.points.len()
                || selection.faces.len() != self.faces.len()
            {
                return Err(MlodError::InvalidSelection {
                    name: name.to_owned(),
                    points: selection.points.len(),
                    faces: selection.faces.len(),
                });
            }
            data.extend(&selection.points);
            data.extend(&selection.faces);
            Ok(())
        };
        let name = match tagg {
            MlodTagg::SharpEdges(edges) => {
                for edge in edges {
                    data.extend(edge[0].to_le_bytes());
                    data.extend(edge[1].to_le_bytes());
                }
                SHARP_EDGES
            }
            MlodTagg::Property { name, value } => {
                data.extend(property_bytes(name)?);
                data.extend(property_bytes(value)?);
                PROPERTY
            }
            MlodTagg::Mass(masses) => {
                Self::check_tagg_len(MASS, masses.len(), self.points.len())?;
                for mass in masses {
                    data.extend(mass.to_le_bytes());
                }
                MASS
            }
            MlodTagg::UvSet { id, uvs } => {
                Self::check_tagg_len(UV_SET, uvs.len(), self.corners())?;
                data.extend(id.to_le_bytes());
                for uv in uvs {
                    data.extend(uv[0].to_le_bytes());
                    data.extend(uv[1].to_le_bytes());
                }
                UV_SET
            }
            MlodTagg::Animation { time, positions } => {
                Self::check_tagg_len(ANIMATION, positions.len(), self.points.len())?;
                data.extend(time.to_le_bytes());
                for position in positions {
                    put_vec3(&mut data, *position);
                }
                ANIMATION
            }
            MlodTagg::Selected(s) => {
                selection(&mut data, SELECTED, s)?;
                SELECTED
            }
            MlodTagg::Hidden(s) => {
                selection(&mut data, HIDDEN, s)?;
                HIDDEN
            }
            MlodTagg::Lock(s) => {
                selection(&mut data, LOCK, s)?;
                LOCK
            }
            MlodTagg::NamedSelection { name, selection: s } => {
                if name.is_empty() || name.starts_with('#') {
                    return Err(MlodError::InvalidSelectionName(name.clone()));
                }
                selection(&mut data, name, s)?;
                name
            }
            MlodTagg::Unknown { active, name, data } => {
                if *active == 1 && name == END_OF_FILE && data.is_empty() {
                    return Err(MlodError::EndOfFileTagg);
                }
                return put_tagg(out, *active, name, data);
            }
        };
        put_tagg(out, 1, name, &data)
    }

    fn check_tagg_len(name: &str, len: usize, expected: usize) -> Result<(), MlodError> {
        if len == expected {
            Ok(())
        } else {
            Err(MlodError::InvalidTaggLength {
                name: name.to_owned(),
                len,
                expected,
            })
        }
    }

    /// The number of used vertices of all faces, the length of a UV set.
    #[must_use]
    pub fn corners(&self) -> usize {
        self.faces.iter().map(|f| f.used().len()).sum()
    }

    /// The named selections in file order.
    pub fn named_selections(&self) -> impl Iterator<Item = MlodNamedSelection<'_>> {
        self.taggs.iter().filter_map(|tagg| match tagg {
            MlodTagg::NamedSelection { name, selection } => {
                Some(MlodNamedSelection { name, selection })
            }
            _ => None,
        })
    }

    /// The first named selection of a name, ignoring ASCII case like the engine.
    #[must_use]
    pub fn named_selection(&self, name: &str) -> Option<&MlodSelection> {
        self.named_selections()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .map(|s| s.selection)
    }

    /// The named properties in file order.
    pub fn properties(&self) -> impl Iterator<Item = (&str, &str)> {
        self.taggs.iter().filter_map(|tagg| match tagg {
            MlodTagg::Property { name, value } => Some((name.as_str(), value.as_str())),
            _ => None,
        })
    }

    /// The value of the first property of a name, ignoring ASCII case.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }

    /// The texture coordinates of a UV set: one per used vertex of each face, in face order.
    #[must_use]
    pub fn uv_set(&self, id: u32) -> Option<&[[f32; 2]]> {
        self.taggs.iter().find_map(|tagg| match tagg {
            MlodTagg::UvSet { id: i, uvs } if *i == id => Some(uvs.as_slice()),
            _ => None,
        })
    }

    /// The mass of each point, if the LOD has `#Mass#`.
    #[must_use]
    pub fn masses(&self) -> Option<&[f32]> {
        self.taggs.iter().find_map(|tagg| match tagg {
            MlodTagg::Mass(masses) => Some(masses.as_slice()),
            _ => None,
        })
    }

    /// The pairs of point indices of `#SharpEdges#`.
    #[must_use]
    pub fn sharp_edges(&self) -> Option<&[[u32; 2]]> {
        self.taggs.iter().find_map(|tagg| match tagg {
            MlodTagg::SharpEdges(edges) => Some(edges.as_slice()),
            _ => None,
        })
    }
}

impl MlodFace {
    fn parse(input: &mut Reader) -> Result<Self, MlodError> {
        let sides = input.u32()?;
        let mut vertices = [MlodVertex::default(); 4];
        for vertex in &mut vertices {
            *vertex = MlodVertex {
                point: input.u32()?,
                normal: input.u32()?,
                u: input.f32()?,
                v: input.f32()?,
            };
        }
        let face = Self {
            sides,
            vertices,
            flags: input.u32()?,
            texture: input.string()?,
            material: input.string()?,
        };
        if !(3..=4).contains(&sides) {
            return Err(MlodError::InvalidSides(sides));
        }
        Ok(face)
    }

    /// The used vertices, 3 or 4 (all 4 for an invalid [`MlodFace::sides`]).
    #[must_use]
    pub fn used(&self) -> &[MlodVertex] {
        &self.vertices[..(self.sides as usize).min(4)]
    }
}

impl MlodSelection {
    /// The weight of a point, `0.0` when it isn't selected.
    #[must_use]
    pub fn weight(&self, point: usize) -> f32 {
        self.points
            .get(point)
            .copied()
            .map_or(0.0, weight_from_byte)
    }

    /// Whether a face is selected.
    #[must_use]
    pub fn has_face(&self, face: usize) -> bool {
        self.faces.get(face).is_some_and(|&b| b != 0)
    }
}

/// The weight of a selection point byte: `0` is `0.0`, else `(256 - byte) / 255`.
#[must_use]
pub fn weight_from_byte(byte: u8) -> f32 {
    if byte == 0 {
        0.0
    } else {
        f32::from(256 - u16::from(byte)) / 255.0
    }
}

/// The selection point byte of a weight like Object Builder: clamped to `0.0..=1.0`, `256` minus
/// the weight times 255 rounded half to even, so weights below `0.5 / 255` aren't selected.
#[must_use]
pub fn weight_to_byte(weight: f32) -> u8 {
    // NaN isn't selected.
    let steps = (weight.clamp(0.0, 1.0) * 255.0).round_ties_even();
    if steps.is_nan() || steps < 1.0 {
        0
    } else {
        // 1 to 255.
        (256 - steps as u16) as u8
    }
}

fn property_string(bytes: &[u8]) -> Result<String, MlodError> {
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8(bytes[..len].to_vec()).map_err(|e| MlodError::InvalidString(e.into_bytes()))
}

fn property_bytes(value: &str) -> Result<[u8; PROPERTY_LEN], MlodError> {
    let mut bytes = [0; PROPERTY_LEN];
    if value.len() >= PROPERTY_LEN || value.contains('\0') {
        return Err(MlodError::InvalidProperty(value.to_owned()));
    }
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    Ok(bytes)
}

fn put_tagg(out: &mut Vec<u8>, active: u8, name: &str, data: &[u8]) -> Result<(), MlodError> {
    out.push(active);
    put_string(out, name)?;
    put_len(out, data.len())?;
    out.extend(data);
    Ok(())
}

fn put_len(out: &mut Vec<u8>, len: usize) -> Result<(), MlodError> {
    let len = u32::try_from(len).map_err(|_| MlodError::TooLarge)?;
    out.extend(len.to_le_bytes());
    Ok(())
}

fn put_vec3(out: &mut Vec<u8>, v: [f32; 3]) {
    for c in v {
        out.extend(c.to_le_bytes());
    }
}

fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), MlodError> {
    if value.contains('\0') {
        return Err(MlodError::InvalidName(value.to_owned()));
    }
    out.extend(value.as_bytes());
    out.push(0);
    Ok(())
}

/// The data is in memory, so reads only fail at its end.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    const fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], MlodError> {
        let bytes = self
            .pos
            .checked_add(len)
            .and_then(|end| self.bytes.get(self.pos..end))
            .ok_or(MlodError::Truncated)?;
        self.pos += len;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, MlodError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, MlodError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }

    fn f32(&mut self) -> Result<f32, MlodError> {
        Ok(f32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }

    fn vec3(&mut self) -> Result<[f32; 3], MlodError> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    fn string(&mut self) -> Result<String, MlodError> {
        let rest = &self.bytes[self.pos..];
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(MlodError::Truncated)?;
        let bytes = self.take(len + 1)?;
        String::from_utf8(bytes[..len].to_vec())
            .map_err(|e| MlodError::InvalidString(e.into_bytes()))
    }
}
