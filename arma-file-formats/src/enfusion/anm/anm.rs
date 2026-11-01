use std::io::{Cursor, Read, Seek, Write};

use binrw::{BinRead, BinWrite, binrw, io::NoSeek};

use crate::{
    core::iff::FormHeader,
    enfusion::ent::Ent,
    errors::{AffError, AnmError},
};

const ANM_FORM_TYPE: &[u8; 4] = b"ANIM";

const FLAG_ADDITIVE: u8 = 0x01;
/// SET6 only: the translation, rotation track has no keys.
const FLAG_NO_TRANSLATION: u8 = 0x02;
const FLAG_NO_ROTATION: u8 = 0x04;
const FLAG_FULL_TRANSLATION: u8 = 0x10;
const FLAG_FULL_ROTATION: u8 = 0x20;
const FLAG_FULL_SCALE: u8 = 0x40;
const FLAGS_SET7: u8 = FLAG_ADDITIVE | FLAG_FULL_TRANSLATION | FLAG_FULL_ROTATION | FLAG_FULL_SCALE;
const FLAGS_SET6: u8 = FLAGS_SET7 | FLAG_NO_TRANSLATION | FLAG_NO_ROTATION;
const BONE_SIZE: usize = 32;

/// Enfusion animation (`.anm`), see the [module docs](super).
#[derive(Debug, Clone, PartialEq)]
pub struct Anm {
    pub version: AnmVersion,
    /// Number of frames, keys are on frames `0..frames`. One more than `#numFrames` of the TXA.
    pub frames: u16,
    /// Frames per second, 30 in all game files.
    pub fps: u16,
    pub bones: Vec<AnmBone>,
    pub events: Option<AnmEvents>,
    /// `CPRP`: custom properties of the export, e.g. `profile` and `sourceFile`, in file order.
    pub properties: Option<Vec<(String, String)>>,
    /// Chunks of the animation set this reader does not know, in file order. They are written
    /// after the known chunks.
    pub unknown_chunks: Vec<AnmChunk>,
}

/// The animation set chunk, the version of the layout. Workbench also loads `SET4` and `SET5`,
/// which this reader doesn't know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnmVersion {
    /// Built by Workbench until 2025, e.g. in mods and BI's samples.
    Set6,
    /// All game files, built by the current Workbench.
    Set7,
}

impl AnmVersion {
    #[must_use]
    pub const fn tag(self) -> &'static [u8; 4] {
        match self {
            Self::Set6 => b"SET6",
            Self::Set7 => b"SET7",
        }
    }
}

/// The events of an animation. SET7 files have `EVN3`, SET6 files `EVN1` or, in BI's samples
/// only for events with objects, `EVN2`. The writer writes the chunk of the variant.
#[derive(Debug, Clone, PartialEq)]
pub enum AnmEvents {
    /// `EVN3` (SET7): an EBIN document, the format of binary worlds, with one top level object
    /// per event, e.g. an `AnimSrcEventGeneric` with `Name`, `Frame` and `FrameCount`.
    Objects(Ent),
    /// `EVN2` (SET6): the frame and frame count of every event, then an EBIN document of their
    /// objects, which have no `Frame` and `FrameCount` properties.
    FramedObjects {
        /// Frame and frame count, one per object.
        frames: Vec<[u32; 2]>,
        objects: Ent,
    },
    /// `EVN1` (SET6): events without object.
    Plain(Vec<AnmPlainEvent>),
}

/// An event of an `EVN1` chunk, in the TXA
/// `#event <frame> "<name>" "<user string>" <user int> <frame count> "<comment>" <main path only>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnmPlainEvent {
    pub frame: u32,
    /// E.g. `Sound`.
    pub name: String,
    /// E.g. the sound event, `SOUND_VEHICLE_LIGHT_ON`.
    pub user_string: String,
    pub user_int: i32,
    pub frame_count: u32,
    pub comment: String,
    /// `0` or `1`.
    pub main_path_only: u16,
}

/// The tracks of one bone. Bones are matched to the skeleton by name, the file has no hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct AnmBone {
    pub name: String,
    /// The keys are added to the underlying pose (`$nodeDiff` in the TXA), used for upper body,
    /// head and arm bones of additive animations.
    pub additive: bool,
    /// Flag bits this reader does not know, `0` in all game files.
    pub unknown_flags: u8,
    pub translation: AnmTrack<3>,
    /// Quaternions `x y z w`.
    pub rotation: AnmTrack<4>,
    pub scale: AnmTrack<3>,
}

/// Quantized keys of a translation, rotation or scale track: a component is
/// `min + value * step`. One range is shared by all components of all keys.
///
/// Workbench drops the keys that interpolation reproduces (within about `1e-4`), so a track may
/// have keys on a few frames only, see [`AnmTrack::sample`].
#[derive(Debug, Clone, PartialEq)]
pub struct AnmTrack<const N: usize> {
    pub min: f32,
    /// `(max - min) / 65535` of the components. Workbench takes the range of the values on all
    /// frames before it drops keys, so it may be wider than the range of the keys.
    pub step: f32,
    /// SET6 stores the range, `step * 65535`. Below 1 these products are 2 ulp apart, so a few
    /// ranges can't be given back by any `step`: then the range as read, which the writer uses
    /// as long as `step` is still its step. `None` otherwise.
    pub set6_range: Option<f32>,
    /// Sorted by frame. A track without keys keeps the bone's pose of the skeleton.
    pub keys: Vec<AnmKey<N>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnmKey<const N: usize> {
    pub frame: u16,
    pub value: [u16; N],
}

/// A chunk of the animation set, with a big endian tag and size.
#[binrw]
#[brw(big)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnmChunk {
    pub tag: [u8; 4],
    #[br(temp)]
    #[bw(try_calc = u32::try_from(data.len()))]
    size: u32,
    #[br(count = size)]
    pub data: Vec<u8>,
}

impl<const N: usize> AnmTrack<N> {
    /// The value of a key.
    #[must_use]
    pub fn decode(&self, key: &AnmKey<N>) -> [f32; N] {
        key.value.map(|v| f32::from(v).mul_add(self.step, self.min))
    }

    /// Frames and values of all keys.
    pub fn values(&self) -> impl Iterator<Item = (u16, [f32; N])> + '_ {
        self.keys.iter().map(|key| (key.frame, self.decode(key)))
    }

    /// Quantizes keys, given by frame and value and sorted by frame, with the range of their
    /// components and rounding, like the current Workbench (SET7) does for a track it keeps all
    /// keys of. SET6 Workbench truncated instead. With equal components
    /// the step is `0`, without keys also the minimum. For the ranges Workbench gives tracks
    /// without keys see [`AnmBone::new`].
    #[must_use]
    pub fn quantize(keys: &[(u16, [f32; N])]) -> Self {
        let components = || keys.iter().flat_map(|(_, value)| value.iter().copied());
        let min = components().fold(f32::INFINITY, f32::min);
        let max = components().fold(f32::NEG_INFINITY, f32::max);
        if keys.is_empty() {
            return Self {
                min: 0.0,
                step: 0.0,
                set6_range: None,
                keys: Vec::new(),
            };
        }
        let step = (max - min) / 65535.0;
        let quantize = |v: f32| {
            if step > 0.0 {
                // `as` saturates, rounding errors can't wrap.
                ((v - min) / step).round() as u16
            } else {
                0
            }
        };
        Self {
            min,
            step,
            set6_range: None,
            keys: keys
                .iter()
                .map(|&(frame, value)| AnmKey {
                    frame,
                    value: value.map(quantize),
                })
                .collect(),
        }
    }

    /// The keys before and after `frame` and the position between them, `0..1`. Before the
    /// first and after the last key both are that key.
    fn segment(&self, frame: f32) -> Option<(&AnmKey<N>, &AnmKey<N>, f32)> {
        let first = self.keys.first()?;
        let last = self.keys.last()?;
        if frame <= f32::from(first.frame) {
            return Some((first, first, 0.0));
        }
        let next = self.keys.partition_point(|k| f32::from(k.frame) <= frame);
        if next == self.keys.len() {
            return Some((last, last, 0.0));
        }
        let (a, b) = (&self.keys[next - 1], &self.keys[next]);
        let t = (frame - f32::from(a.frame)) / f32::from(b.frame - a.frame);
        Some((a, b, t))
    }

    /// The range of SET6, `step * 65535` or the [`AnmTrack::set6_range`] of `step`.
    fn range(&self) -> f32 {
        match self.set6_range {
            Some(range) if step_from_range(range).0 == self.step => range,
            _ => self.step * 65535.0,
        }
    }

    /// Whether the track has a key on every frame. The file stores no frame numbers then.
    fn is_full(&self, frames: u16) -> bool {
        !self.keys.is_empty() && self.keys.len() == usize::from(frames)
    }

    fn read(&mut self, data: &mut Reader, frames: u16) -> Option<()> {
        let count = self.keys.len();
        let indexed = count > 0 && count < usize::from(frames);
        let mut key_frames = Vec::with_capacity(count);
        for i in 0..count {
            // A full track has a key on every frame.
            key_frames.push(if indexed { data.u16()? } else { i as u16 });
        }
        for (key, frame) in self.keys.iter_mut().zip(key_frames) {
            key.frame = frame;
            for v in &mut key.value {
                *v = data.u16()?;
            }
        }
        Some(())
    }

    fn write(&self, out: &mut Vec<u8>, frames: u16) -> Result<(), AnmError> {
        let count = self.keys.len();
        if count > usize::from(frames) {
            return Err(AnmError::Unwritable("DATA"));
        }
        if count > 0 && count < usize::from(frames) {
            for key in &self.keys {
                out.extend(key.frame.to_le_bytes());
            }
        } else if self
            .keys
            .iter()
            .enumerate()
            .any(|(i, k)| usize::from(k.frame) != i)
        {
            // A key on every frame, the frames are implicit.
            return Err(AnmError::Unwritable("DATA"));
        }
        for key in &self.keys {
            for v in key.value {
                out.extend(v.to_le_bytes());
            }
        }
        Ok(())
    }
}

impl AnmTrack<3> {
    /// The translation or scale at a frame, which may lie between frames: linear between keys,
    /// the first or last key before or after them. `None` without keys.
    ///
    /// This is the interpolation Workbench assumes when it drops keys.
    #[must_use]
    pub fn sample(&self, frame: f32) -> Option<[f32; 3]> {
        let (a, b, t) = self.segment(frame)?;
        let (a, b) = (self.decode(a), self.decode(b));
        Some(std::array::from_fn(|i| (b[i] - a[i]).mul_add(t, a[i])))
    }
}

impl AnmTrack<4> {
    /// The rotation at a frame, which may lie between frames: spherical linear interpolation
    /// along the shorter arc between keys, the first or last key before or after them. The
    /// result is normalized. `None` without keys.
    ///
    /// This is the interpolation Workbench assumes when it drops keys.
    #[must_use]
    pub fn sample(&self, frame: f32) -> Option<[f32; 4]> {
        let (a, b, t) = self.segment(frame)?;
        let (a, mut b) = (normalize(self.decode(a)), normalize(self.decode(b)));
        let mut dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        if dot < 0.0 {
            b = b.map(|v| -v);
            dot = -dot;
        }
        let (wa, wb) = if dot > 0.9995 {
            // Nearly equal, lerp avoids dividing by a tiny sine.
            (1.0 - t, t)
        } else {
            let angle = dot.min(1.0).acos();
            let sin = angle.sin();
            (((1.0 - t) * angle).sin() / sin, (t * angle).sin() / sin)
        };
        Some(normalize(std::array::from_fn(|i| {
            a[i].mul_add(wa, b[i] * wb)
        })))
    }
}

fn normalize(q: [f32; 4]) -> [f32; 4] {
    let length = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length > 0.0 {
        q.map(|v| v / length)
    } else {
        q
    }
}

impl AnmBone {
    /// A bone without keys, with the ranges Workbench gives such tracks: `0` and `1 / 65535`
    /// for translation and rotation, `1` and `0` for scale.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        const fn empty<const N: usize>(min: f32, step: f32) -> AnmTrack<N> {
            AnmTrack {
                min,
                step,
                set6_range: None,
                keys: Vec::new(),
            }
        }
        Self {
            name: name.into(),
            additive: false,
            unknown_flags: 0,
            translation: empty(0.0, 1.0 / 65535.0),
            rotation: empty(0.0, 1.0 / 65535.0),
            scale: empty(1.0, 0.0),
        }
    }
}

impl Anm {
    pub fn from<I>(input: &mut I) -> Result<Self, AffError>
    where
        I: Seek + Read,
    {
        let start = input.stream_position()?;
        let form = FormHeader::read_form(input)?;
        if &form.form_type != ANM_FORM_TYPE {
            return Err(AnmError::UnknownFormType(form.form_type_lossy()).into());
        }
        let form_end = form.end(start);
        let set = AnmChunk::read(input)?;
        let version = match &set.tag {
            b"SET6" => AnmVersion::Set6,
            b"SET7" => AnmVersion::Set7,
            tag => {
                return Err(AnmError::UnsupportedSet(String::from_utf8_lossy(tag).into()).into());
            }
        };
        if input.stream_position()? != form_end {
            return Err(AnmError::InvalidChunk("ANIM").into());
        }

        let mut set = Cursor::new(set.data);
        let end = set.get_ref().len() as u64;
        let mut chunks = Vec::new();
        while set.position() < end {
            chunks.push(AnmChunk::read(&mut set)?);
        }
        let mut take = |tag: &[u8; 4]| {
            chunks
                .iter()
                .position(|c| &c.tag == tag)
                .map(|i| chunks.remove(i))
        };
        let fps = match version {
            AnmVersion::Set6 => Some(
                take(b"FPS\0")
                    .ok_or(AnmError::ChunkMissing("FPS"))?
                    .data
                    .try_into()
                    .ok()
                    .map(u32::from_le_bytes)
                    .and_then(|fps| u16::try_from(fps).ok())
                    .ok_or(AnmError::InvalidChunk("FPS"))?,
            ),
            AnmVersion::Set7 => None,
        };
        let head = take(b"HEAD").ok_or(AnmError::ChunkMissing("HEAD"))?;
        let data = take(b"DATA").ok_or(AnmError::ChunkMissing("DATA"))?;
        let events = if let Some(c) = take(b"EVN3") {
            Some(AnmEvents::Objects(read_evn3(&c.data)?))
        } else if let Some(c) = take(b"EVN2") {
            Some(read_evn2(&c.data)?)
        } else if let Some(c) = take(b"EVN1") {
            Some(AnmEvents::Plain(
                read_evn1(&c.data).ok_or(AnmError::InvalidChunk("EVN1"))?,
            ))
        } else {
            None
        };
        let properties = take(b"CPRP")
            .map(|c| read_properties(&c.data).ok_or(AnmError::InvalidChunk("CPRP")))
            .transpose()?;

        let (frames, fps, mut bones) = match fps {
            Some(fps) => read_head6(&head.data).map(|(frames, bones)| (frames, fps, bones)),
            None => read_head7(&head.data),
        }
        .ok_or(AnmError::InvalidChunk("HEAD"))?;
        let mut reader = Reader::new(&data.data);
        for bone in &mut bones {
            bone.translation
                .read(&mut reader, frames)
                .and_then(|()| bone.scale.read(&mut reader, frames))
                .and_then(|()| bone.rotation.read(&mut reader, frames))
                .ok_or(AnmError::InvalidChunk("DATA"))?;
        }
        if !reader.is_empty() {
            return Err(AnmError::InvalidChunk("DATA").into());
        }

        Ok(Self {
            version,
            frames,
            fps,
            bones,
            events,
            properties,
            unknown_chunks: chunks,
        })
    }

    /// Writes the animation at the current position of `out`, in the layout of
    /// [`Anm::version`].
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let chunk = |tag: &[u8; 4], data| AnmChunk { tag: *tag, data };
        let mut chunks = match self.version {
            AnmVersion::Set6 => vec![
                chunk(b"FPS\0", u32::from(self.fps).to_le_bytes().to_vec()),
                chunk(b"HEAD", self.head6()?),
            ],
            AnmVersion::Set7 => vec![chunk(b"HEAD", self.head7()?)],
        };
        chunks.push(chunk(b"DATA", self.data()?));
        match &self.events {
            Some(AnmEvents::Objects(events)) => chunks.push(chunk(b"EVN3", write_evn3(events)?)),
            Some(AnmEvents::FramedObjects { frames, objects }) => {
                chunks.push(chunk(b"EVN2", write_evn2(frames, objects)?));
            }
            Some(AnmEvents::Plain(events)) => chunks.push(chunk(b"EVN1", write_evn1(events)?)),
            None => {}
        }
        if let Some(properties) = &self.properties {
            chunks.push(chunk(b"CPRP", write_properties(properties)?));
        }

        let mut set = Cursor::new(Vec::new());
        for chunk in chunks.iter().chain(&self.unknown_chunks) {
            chunk.write(&mut set)?;
        }
        let set = AnmChunk {
            tag: *self.version.tag(),
            data: set.into_inner(),
        };
        let mut body = Cursor::new(Vec::new());
        set.write(&mut body)?;
        let body = body.into_inner();
        FormHeader {
            size: u32::try_from(body.len() + 4).map_err(|_| AnmError::TooLarge)?,
            form_type: *ANM_FORM_TYPE,
        }
        .write(&mut NoSeek::new(&mut *out))?;
        out.write_all(&body)?;
        Ok(())
    }

    /// The bone with this name, matched ASCII case insensitively like the engine's bone names.
    #[must_use]
    pub fn bone(&self, name: &str) -> Option<&AnmBone> {
        self.bones
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(name))
    }

    /// Length in seconds, from the first to the last frame.
    #[must_use]
    pub fn duration(&self) -> f32 {
        if self.fps == 0 {
            return 0.0;
        }
        f32::from(self.frames.saturating_sub(1)) / f32::from(self.fps)
    }

    /// The flags byte of a bone in the HEAD of this version.
    fn flags(&self, bone: &AnmBone) -> u8 {
        let known = match self.version {
            AnmVersion::Set6 => FLAGS_SET6,
            AnmVersion::Set7 => FLAGS_SET7,
        };
        let mut flags = bone.unknown_flags & !known;
        let mut set = |flag: u8, on: bool| {
            if on && known & flag != 0 {
                flags |= flag;
            }
        };
        set(FLAG_ADDITIVE, bone.additive);
        set(FLAG_NO_TRANSLATION, bone.translation.keys.is_empty());
        set(FLAG_NO_ROTATION, bone.rotation.keys.is_empty());
        set(FLAG_FULL_TRANSLATION, bone.translation.is_full(self.frames));
        set(FLAG_FULL_ROTATION, bone.rotation.is_full(self.frames));
        set(FLAG_FULL_SCALE, bone.scale.is_full(self.frames));
        flags
    }

    /// Key counts of a bone, translation, rotation and scale.
    fn counts(bone: &AnmBone) -> Result<[u16; 3], AnmError> {
        let count = |n: usize| u16::try_from(n).map_err(|_| AnmError::Unwritable("HEAD"));
        Ok([
            count(bone.translation.keys.len())?,
            count(bone.rotation.keys.len())?,
            count(bone.scale.keys.len())?,
        ])
    }

    /// SET7: frames, fps, bone count, the names, then a fixed size entry per bone.
    fn head7(&self) -> Result<Vec<u8>, AnmError> {
        let unwritable = |_| AnmError::Unwritable("HEAD");
        let mut names = Vec::new();
        for bone in &self.bones {
            if bone.name.contains('\0') {
                return Err(AnmError::Unwritable("HEAD"));
            }
            names.extend(bone.name.as_bytes());
            names.push(0);
        }
        let mut out = Vec::with_capacity(8 + names.len() + self.bones.len() * BONE_SIZE);
        out.extend(self.frames.to_le_bytes());
        out.extend(self.fps.to_le_bytes());
        out.extend(
            u16::try_from(self.bones.len())
                .map_err(unwritable)?
                .to_le_bytes(),
        );
        out.extend(
            u16::try_from(names.len())
                .map_err(unwritable)?
                .to_le_bytes(),
        );
        out.extend(names);
        for bone in &self.bones {
            for count in Self::counts(bone)? {
                out.extend(count.to_le_bytes());
            }
            out.push(self.flags(bone));
            out.push(u8::try_from(bone.name.len() + 1).map_err(unwritable)?);
            for v in [
                bone.translation.min,
                bone.translation.step,
                bone.rotation.min,
                bone.rotation.step,
                bone.scale.min,
                bone.scale.step,
            ] {
                out.extend(v.to_le_bytes());
            }
        }
        Ok(out)
    }

    /// SET6: per bone the ranges, frames, counts, flags and the name.
    fn head6(&self) -> Result<Vec<u8>, AnmError> {
        let mut out = Vec::new();
        for bone in &self.bones {
            for (min, range) in [
                (bone.translation.min, bone.translation.range()),
                (bone.rotation.min, bone.rotation.range()),
                (bone.scale.min, bone.scale.range()),
            ] {
                out.extend(min.to_le_bytes());
                out.extend(range.to_le_bytes());
            }
            out.extend(self.frames.to_le_bytes());
            for count in Self::counts(bone)? {
                out.extend(count.to_le_bytes());
            }
            out.push(self.flags(bone));
            out.push(u8::try_from(bone.name.len()).map_err(|_| AnmError::Unwritable("HEAD"))?);
            out.extend(bone.name.as_bytes());
        }
        Ok(out)
    }

    fn data(&self) -> Result<Vec<u8>, AnmError> {
        let mut out = Vec::new();
        for bone in &self.bones {
            bone.translation.write(&mut out, self.frames)?;
            bone.scale.write(&mut out, self.frames)?;
            bone.rotation.write(&mut out, self.frames)?;
        }
        Ok(out)
    }
}

/// Minimum, step and [`AnmTrack::set6_range`] of a track.
type Range = (f32, f32, Option<f32>);

/// A bone of the HEAD, its tracks with keys of unknown frames and values.
fn bone(name: String, flags: u8, known: u8, counts: [u16; 3], ranges: [Range; 3]) -> AnmBone {
    fn track<const N: usize>((min, step, set6_range): Range, count: u16) -> AnmTrack<N> {
        let key = AnmKey {
            frame: 0,
            value: [0; N],
        };
        AnmTrack {
            min,
            step,
            set6_range,
            keys: vec![key; usize::from(count)],
        }
    }
    AnmBone {
        name,
        additive: flags & FLAG_ADDITIVE != 0,
        unknown_flags: flags & !known,
        translation: track(ranges[0], counts[0]),
        rotation: track(ranges[1], counts[1]),
        scale: track(ranges[2], counts[2]),
    }
}

fn read_head7(data: &[u8]) -> Option<(u16, u16, Vec<AnmBone>)> {
    let mut r = Reader::new(data);
    let frames = r.u16()?;
    let fps = r.u16()?;
    let count = usize::from(r.u16()?);
    let names_size = usize::from(r.u16()?);
    let names = r.bytes(names_size)?;
    let mut names = names.strip_suffix(&[0])?.split(|&b| b == 0);

    let mut bones = Vec::with_capacity(count);
    for _ in 0..count {
        let name = String::from_utf8(names.next()?.to_vec()).ok()?;
        let counts = [r.u16()?, r.u16()?, r.u16()?];
        let flags = r.u8()?;
        if usize::from(r.u8()?) != name.len() + 1 {
            return None;
        }
        let ranges = [(); 3].map(|()| Some((r.f32()?, r.f32()?, None)));
        let ranges = [ranges[0]?, ranges[1]?, ranges[2]?];
        bones.push(bone(name, flags, FLAGS_SET7, counts, ranges));
    }
    (names.next().is_none() && r.is_empty()).then_some((frames, fps, bones))
}

/// SET6 has no bone count, the entries fill the chunk. Every bone stores the frame count.
fn read_head6(data: &[u8]) -> Option<(u16, Vec<AnmBone>)> {
    let mut r = Reader::new(data);
    let mut frames = None;
    let mut bones = Vec::new();
    while !r.is_empty() {
        let ranges = [(); 3].map(|()| {
            let min = r.f32()?;
            let (step, set6_range) = step_from_range(r.f32()?);
            Some((min, step, set6_range))
        });
        let ranges = [ranges[0]?, ranges[1]?, ranges[2]?];
        let bone_frames = r.u16()?;
        if *frames.get_or_insert(bone_frames) != bone_frames {
            return None;
        }
        let counts = [r.u16()?, r.u16()?, r.u16()?];
        let flags = r.u8()?;
        let name_len = usize::from(r.u8()?);
        let name = String::from_utf8(r.bytes(name_len)?.to_vec()).ok()?;
        bones.push(bone(name, flags, FLAGS_SET6, counts, ranges));
    }
    Some((frames.unwrap_or(0), bones))
}

/// The step of a SET6 range, chosen so that `step * 65535` gives the range back, and the range
/// if no step does.
fn step_from_range(range: f32) -> (f32, Option<f32>) {
    let step = range / 65535.0;
    [step, step.next_up(), step.next_down()]
        .into_iter()
        .find(|s| s * 65535.0 == range)
        .map_or((step, Some(range)), |step| (step, None))
}

/// `EVN3`: the number of events, the size of the EBIN document and the document.
fn read_evn3(data: &[u8]) -> Result<Ent, AffError> {
    let mut r = Reader::new(data);
    let count = r.u16().ok_or(AnmError::InvalidChunk("EVN3"))?;
    read_ebin(&mut r, count, "EVN3")
}

/// `EVN2`: the number of events, `u32` frame and frame count per event, then the size of the
/// EBIN document and the document.
fn read_evn2(data: &[u8]) -> Result<AnmEvents, AffError> {
    let invalid = || AnmError::InvalidChunk("EVN2");
    let mut r = Reader::new(data);
    let count = r.u16().ok_or_else(invalid)?;
    let frames = (0..count)
        .map(|_| Some([r.u32()?, r.u32()?]))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    let objects = read_ebin(&mut r, count, "EVN2")?;
    Ok(AnmEvents::FramedObjects { frames, objects })
}

/// The rest of an event chunk: a `u16` size and an EBIN document with `count` top level
/// entries.
fn read_ebin(r: &mut Reader, count: u16, tag: &'static str) -> Result<Ent, AffError> {
    let invalid = || AnmError::InvalidChunk(tag);
    let size = r.u16().ok_or_else(invalid)?;
    let blob = r.bytes(usize::from(size)).ok_or_else(invalid)?;
    let objects = Ent::from(&mut Cursor::new(blob))?;
    if objects.entries.len() != usize::from(count) || !r.is_empty() {
        return Err(invalid().into());
    }
    Ok(objects)
}

fn write_ebin(objects: &Ent, tag: &'static str, out: &mut Vec<u8>) -> Result<(), AffError> {
    let mut blob = Vec::new();
    objects.write(&mut blob)?;
    let size = u16::try_from(blob.len()).map_err(|_| AnmError::Unwritable(tag))?;
    out.extend(size.to_le_bytes());
    out.extend(blob);
    Ok(())
}

fn count_u16(count: usize, tag: &'static str) -> Result<[u8; 2], AnmError> {
    u16::try_from(count)
        .map(u16::to_le_bytes)
        .map_err(|_| AnmError::Unwritable(tag))
}

fn write_evn3(objects: &Ent) -> Result<Vec<u8>, AffError> {
    let mut out = count_u16(objects.entries.len(), "EVN3")?.to_vec();
    write_ebin(objects, "EVN3", &mut out)?;
    Ok(out)
}

fn write_evn2(frames: &[[u32; 2]], objects: &Ent) -> Result<Vec<u8>, AffError> {
    if frames.len() != objects.entries.len() {
        return Err(AnmError::Unwritable("EVN2").into());
    }
    let mut out = count_u16(frames.len(), "EVN2")?.to_vec();
    for v in frames.iter().flatten() {
        out.extend(v.to_le_bytes());
    }
    write_ebin(objects, "EVN2", &mut out)?;
    Ok(out)
}

fn read_evn1(data: &[u8]) -> Option<Vec<AnmPlainEvent>> {
    let mut r = Reader::new(data);
    let count = r.u16()?;
    let events = (0..count)
        .map(|_| {
            Some(AnmPlainEvent {
                frame: r.u32()?,
                name: r.string()?,
                user_string: r.string()?,
                user_int: r.u32()? as i32,
                frame_count: r.u32()?,
                comment: r.string()?,
                main_path_only: r.u16()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    r.is_empty().then_some(events)
}

fn write_evn1(events: &[AnmPlainEvent]) -> Result<Vec<u8>, AnmError> {
    let mut out = count_u16(events.len(), "EVN1")?.to_vec();
    for event in events {
        out.extend(event.frame.to_le_bytes());
        write_string(&mut out, &event.name, "EVN1")?;
        write_string(&mut out, &event.user_string, "EVN1")?;
        out.extend(event.user_int.to_le_bytes());
        out.extend(event.frame_count.to_le_bytes());
        write_string(&mut out, &event.comment, "EVN1")?;
        out.extend(event.main_path_only.to_le_bytes());
    }
    Ok(out)
}

/// A `u32` length (with the terminating NUL) and a NUL terminated string.
fn write_string(out: &mut Vec<u8>, string: &str, tag: &'static str) -> Result<(), AnmError> {
    if string.contains('\0') {
        return Err(AnmError::Unwritable(tag));
    }
    let len = u32::try_from(string.len() + 1).map_err(|_| AnmError::Unwritable(tag))?;
    out.extend(len.to_le_bytes());
    out.extend(string.as_bytes());
    out.push(0);
    Ok(())
}

/// `CPRP`: the number of properties, then key and value as `u32` length (with the terminating
/// NUL) and NUL terminated string.
fn read_properties(data: &[u8]) -> Option<Vec<(String, String)>> {
    let mut r = Reader::new(data);
    let count = r.u16()?;
    let properties = (0..count)
        .map(|_| Some((r.string()?, r.string()?)))
        .collect::<Option<Vec<_>>>()?;
    r.is_empty().then_some(properties)
}

fn write_properties(properties: &[(String, String)]) -> Result<Vec<u8>, AnmError> {
    let mut out = count_u16(properties.len(), "CPRP")?.to_vec();
    for string in properties.iter().flat_map(|(k, v)| [k, v]) {
        write_string(&mut out, string, "CPRP")?;
    }
    Ok(out)
}

/// Little endian values from a chunk, `None` past the end.
struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        let (bytes, rest) = self.data.split_at_checked(len)?;
        self.data = rest;
        Some(bytes)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.bytes(N)?.try_into().ok()
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.array::<1>()?[0])
    }

    /// A `u32` length (with the terminating NUL) and a NUL terminated UTF-8 string.
    fn string(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        let bytes = self.bytes(len)?.strip_suffix(&[0])?;
        String::from_utf8(bytes.to_vec()).ok()
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.array()?))
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.array()?))
    }
}
