use indexmap::IndexSet;

use super::ent::{Body, Ent, Entry, Object, ObjectId, Value};
use crate::errors::EntError;

pub const ENT_MAGIC: &[u8; 4] = b"EBIN";

const INSTANCE: u8 = 0x00;
const INT: u8 = 0x01;
const FLAGS: u8 = 0x02;
const RESOURCE: u8 = 0x05;
const FLOAT: u8 = 0x06;
const VEC2: u8 = 0x07;
const VEC3: u8 = 0x08;
const VEC4: u8 = 0x09;
const BOOL: u8 = 0x0b;
const STRING: u8 = 0x0c;
const WORD: u8 = 0x0d;
const BLOCK: u8 = 0x0e;
const NAME: u8 = 0x43;
const INLINE_ID: u8 = 0x44;
const SHARED_ID: u8 = 0x45;
const ARRAY: u8 = 0x80;

/// Marks the parent of an object, `Class : "{GUID}" {`.
const COLON: &str = ":";

const fn is_value(tag: u8) -> bool {
    matches!(
        tag & !ARRAY,
        INT | FLAGS | RESOURCE | FLOAT | VEC2 | VEC3 | VEC4 | BOOL | STRING
    )
}

/// Whether a block starting with `tag` is a token stream.
const fn starts_entries(tag: u8) -> bool {
    matches!(tag, INSTANCE | WORD | BLOCK | NAME | INLINE_ID | SHARED_ID) || is_value(tag)
}

pub fn read_ent(data: &[u8]) -> Result<Ent, EntError> {
    if data.get(..4) != Some(ENT_MAGIC) {
        return Err(EntError::UnknownMagic(
            data.get(..4).unwrap_or(data).to_vec(),
        ));
    }
    let mut reader = Reader {
        data,
        pos: 4,
        strings: Vec::new(),
        guids: Vec::new(),
    };
    let version = reader.u32()?;
    let string_count = reader.u16()?;
    let guid_count = reader.u16()?;
    for _ in 0..string_count {
        let string = reader.string()?;
        reader.strings.push(string);
    }
    for _ in 0..guid_count {
        let guid = reader.u64()?;
        reader.guids.push(guid);
    }
    let entries = reader.entries(data.len())?;
    Ok(Ent { version, entries })
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    strings: Vec<String>,
    guids: Vec<u64>,
}

impl Reader<'_> {
    const fn error(&self, message: &'static str) -> EntError {
        EntError::Malformed {
            offset: self.pos,
            message,
        }
    }

    fn bytes(&mut self, len: usize) -> Result<&[u8], EntError> {
        let bytes = self
            .pos
            .checked_add(len)
            .and_then(|end| self.data.get(self.pos..end))
            .ok_or(EntError::Truncated(self.pos))?;
        self.pos += len;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], EntError> {
        let mut array = [0; N];
        array.copy_from_slice(self.bytes(N)?);
        Ok(array)
    }

    fn u8(&mut self) -> Result<u8, EntError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, EntError> {
        self.array().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> Result<u32, EntError> {
        self.array().map(u32::from_le_bytes)
    }

    fn u64(&mut self) -> Result<u64, EntError> {
        self.array().map(u64::from_le_bytes)
    }

    fn f32s<const N: usize>(&mut self) -> Result<[f32; N], EntError> {
        let mut values = [0.0; N];
        for value in &mut values {
            *value = f32::from_le_bytes(self.array()?);
        }
        Ok(values)
    }

    /// `u16` length and UTF-8 bytes.
    fn string(&mut self) -> Result<String, EntError> {
        let start = self.pos;
        let len = self.u16()?;
        let bytes = self.bytes(len.into())?.to_vec();
        String::from_utf8(bytes).map_err(|_| EntError::Malformed {
            offset: start,
            message: "string is not UTF-8",
        })
    }

    fn word(&mut self) -> Result<String, EntError> {
        let index = self.u16()?;
        self.strings
            .get(usize::from(index))
            .cloned()
            .ok_or_else(|| self.error("string index out of range"))
    }

    fn guid(&mut self) -> Result<u64, EntError> {
        let index = self.u16()?;
        self.guids
            .get(usize::from(index))
            .copied()
            .ok_or_else(|| self.error("GUID index out of range"))
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// Whether the next token is a word other than `:`, without consuming it.
    fn peek_class(&self) -> bool {
        self.peek_word().is_some_and(|word| word != COLON)
    }

    fn peek_word(&self) -> Option<&str> {
        if self.peek() != Some(WORD) {
            return None;
        }
        let index = self.data.get(self.pos + 1..self.pos + 3)?;
        let index = u16::from_le_bytes([index[0], index[1]]);
        self.strings.get(usize::from(index)).map(String::as_str)
    }

    fn entries(&mut self, end: usize) -> Result<Vec<Entry>, EntError> {
        let mut entries = Vec::new();
        while self.pos < end {
            entries.push(self.entry(end)?);
        }
        if self.pos != end {
            return Err(self.error("token overruns its block"));
        }
        Ok(entries)
    }

    fn entry(&mut self, end: usize) -> Result<Entry, EntError> {
        let tag = self.peek().ok_or(EntError::Truncated(self.pos))?;
        match tag {
            WORD => {
                self.pos += 1;
                let key = self.word()?;
                match self.peek() {
                    _ if self.pos >= end => Err(self.error("key without value")),
                    Some(WORD | BLOCK | NAME | INLINE_ID | SHARED_ID) => {
                        self.object(Some(key)).map(Entry::Object)
                    }
                    _ => Ok(Entry::Property {
                        key,
                        value: self.value()?,
                    }),
                }
            }
            BLOCK | NAME | INLINE_ID | SHARED_ID => self.object(None).map(Entry::Object),
            INSTANCE => Err(self.error("instance header inside a body")),
            _ => self.value().map(Entry::Value),
        }
    }

    /// The rest of an object after its key: `[class] [name] [id] [: parent] { body }`.
    fn object(&mut self, key: Option<String>) -> Result<Object, EntError> {
        let mut object = Object {
            key,
            ..Object::default()
        };
        if self.peek_class() {
            self.pos += 1;
            object.class = Some(self.word()?);
        }
        if self.peek() == Some(NAME) {
            self.pos += 1;
            object.name = Some(self.string()?);
        }
        match self.peek() {
            Some(INLINE_ID) => {
                self.pos += 1;
                object.id = Some(ObjectId::Inline(self.u64()?));
            }
            Some(SHARED_ID) => {
                self.pos += 1;
                object.id = Some(ObjectId::Shared(self.guid()?));
            }
            _ => {}
        }
        if self.peek_word() == Some(COLON) {
            self.pos += 3;
            if self.u8()? != RESOURCE {
                return Err(self.error("expected the parent resource after `:`"));
            }
            object.parent = Some(self.guid()?);
        }
        if self.u8()? != BLOCK {
            return Err(self.error("expected a block"));
        }
        object.body = self.body()?;
        Ok(object)
    }

    /// `u32` size including itself and the contents.
    fn body(&mut self) -> Result<Body, EntError> {
        let start = self.pos;
        let size = self.u32()? as usize;
        let end = start
            .checked_add(size)
            .filter(|&end| size >= 4 && end <= self.data.len())
            .ok_or(EntError::Malformed {
                offset: start,
                message: "block size out of range",
            })?;
        match self.peek() {
            Some(tag) if self.pos < end && !starts_entries(tag) => {
                let raw = self.bytes(end - self.pos)?.to_vec();
                Ok(Body::Raw(raw))
            }
            tag => {
                let instance = if self.pos < end && tag == Some(INSTANCE) {
                    self.pos += 1;
                    Some(self.u16()?)
                } else {
                    None
                };
                let entries = self.entries(end)?;
                Ok(Body::Entries { instance, entries })
            }
        }
    }

    fn value(&mut self) -> Result<Value, EntError> {
        let start = self.pos;
        let tag = self.u8()?;
        if tag & ARRAY == 0 {
            return self.scalar(tag, start);
        }
        let count = self.u32()? as usize;
        // Every element takes at least a byte, so a bogus count fails before allocating.
        if count > self.data.len() - self.pos {
            return Err(EntError::Truncated(self.pos));
        }
        macro_rules! array {
            ($variant:ident, $read:expr) => {
                Value::$variant((0..count).map(|_| $read(self)).collect::<Result<_, _>>()?)
            };
        }
        Ok(match tag & !ARRAY {
            INT => array!(IntArray, |r: &mut Self| r.array().map(i32::from_le_bytes)),
            FLAGS => array!(FlagsArray, Self::u64),
            RESOURCE => array!(ResourceArray, Self::guid),
            FLOAT => array!(FloatArray, |r: &mut Self| r.f32s::<1>().map(|[v]| v)),
            VEC2 => array!(Vec2Array, Self::f32s),
            VEC3 => array!(Vec3Array, Self::f32s),
            VEC4 => array!(Vec4Array, Self::f32s),
            BOOL => array!(BoolArray, |r: &mut Self| r.u8().map(|b| b != 0)),
            STRING => array!(StringArray, Self::string),
            _ => return Err(EntError::UnknownToken { tag, offset: start }),
        })
    }

    fn scalar(&mut self, tag: u8, start: usize) -> Result<Value, EntError> {
        Ok(match tag {
            INT => Value::Int(i32::from_le_bytes(self.array()?)),
            FLAGS => Value::Flags(self.u64()?),
            RESOURCE => Value::Resource(self.guid()?),
            FLOAT => Value::Float(f32::from_le_bytes(self.array()?)),
            VEC2 => Value::Vec2(self.f32s()?),
            VEC3 => Value::Vec3(self.f32s()?),
            VEC4 => Value::Vec4(self.f32s()?),
            BOOL => Value::Bool(self.u8()? != 0),
            STRING => Value::String(self.string()?),
            _ => return Err(EntError::UnknownToken { tag, offset: start }),
        })
    }
}

pub fn write_ent(ent: &Ent) -> Result<Vec<u8>, EntError> {
    let mut writer = Writer::default();
    writer.entries(&ent.entries)?;

    let mut out = ENT_MAGIC.to_vec();
    out.extend(ent.version.to_le_bytes());
    let count = |len: usize, what| u16::try_from(len).map_err(|_| EntError::TooLarge(what));
    out.extend(count(writer.strings.len(), "string table")?.to_le_bytes());
    out.extend(count(writer.guids.len(), "GUID table")?.to_le_bytes());
    for string in &writer.strings {
        put_string(&mut out, string)?;
    }
    for guid in &writer.guids {
        out.extend(guid.to_le_bytes());
    }
    out.extend(writer.out);
    Ok(out)
}

fn put_string(out: &mut Vec<u8>, string: &str) -> Result<(), EntError> {
    let len = u16::try_from(string.len()).map_err(|_| EntError::TooLarge("string"))?;
    out.extend(len.to_le_bytes());
    out.extend(string.as_bytes());
    Ok(())
}

#[derive(Default)]
struct Writer {
    out: Vec<u8>,
    strings: IndexSet<String>,
    guids: IndexSet<u64>,
}

impl Writer {
    /// Table indices beyond `u16` are caught when writing the table counts.
    fn word(&mut self, word: &str) {
        let index = match self.strings.get_index_of(word) {
            Some(index) => index,
            None => self.strings.insert_full(word.to_owned()).0,
        };
        self.out.push(WORD);
        self.out.extend((index as u16).to_le_bytes());
    }

    fn guid(&mut self, guid: u64) {
        let (index, _) = self.guids.insert_full(guid);
        self.out.extend((index as u16).to_le_bytes());
    }

    fn string(&mut self, string: &str) -> Result<(), EntError> {
        put_string(&mut self.out, string)
    }

    fn entries(&mut self, entries: &[Entry]) -> Result<(), EntError> {
        for entry in entries {
            match entry {
                Entry::Property { key, value } => {
                    self.word(key);
                    self.value(value)?;
                }
                Entry::Object(object) => self.object(object)?,
                Entry::Value(value) => self.value(value)?,
            }
        }
        Ok(())
    }

    fn object(&mut self, object: &Object) -> Result<(), EntError> {
        if let Some(key) = &object.key {
            self.word(key);
        }
        if let Some(class) = &object.class {
            self.word(class);
        }
        if let Some(name) = &object.name {
            self.out.push(NAME);
            self.string(name)?;
        }
        match object.id {
            Some(ObjectId::Inline(id)) => {
                self.out.push(INLINE_ID);
                self.out.extend(id.to_le_bytes());
            }
            Some(ObjectId::Shared(id)) => {
                self.out.push(SHARED_ID);
                self.guid(id);
            }
            None => {}
        }
        if let Some(parent) = object.parent {
            self.word(COLON);
            self.out.push(RESOURCE);
            self.guid(parent);
        }

        self.out.push(BLOCK);
        let start = self.out.len();
        self.out.extend([0; 4]);
        match &object.body {
            Body::Entries { instance, entries } => {
                if let Some(instance) = instance {
                    self.out.push(INSTANCE);
                    self.out.extend(instance.to_le_bytes());
                }
                self.entries(entries)?;
            }
            Body::Raw(raw) => {
                if raw.first().is_some_and(|&tag| starts_entries(tag)) {
                    return Err(EntError::Unwritable(
                        "raw body starting with a token tag reads back as entries",
                    ));
                }
                self.out.extend(raw);
            }
        }
        let size =
            u32::try_from(self.out.len() - start).map_err(|_| EntError::TooLarge("block"))?;
        self.out[start..start + 4].copy_from_slice(&size.to_le_bytes());
        Ok(())
    }

    fn value(&mut self, value: &Value) -> Result<(), EntError> {
        match value {
            Value::Int(v) => self.tag(INT).put(&v.to_le_bytes()),
            Value::Flags(v) => self.tag(FLAGS).put(&v.to_le_bytes()),
            Value::Resource(guid) => self.tag(RESOURCE).guid(*guid),
            Value::Float(v) => self.tag(FLOAT).put(&v.to_le_bytes()),
            Value::Vec2(v) => self.tag(VEC2).f32s(v),
            Value::Vec3(v) => self.tag(VEC3).f32s(v),
            Value::Vec4(v) => self.tag(VEC4).f32s(v),
            Value::Bool(v) => self.tag(BOOL).put(&[u8::from(*v)]),
            Value::String(v) => self.tag(STRING).string(v)?,
            Value::IntArray(v) => self.array(INT, v, |w, v| w.put(&v.to_le_bytes()))?,
            Value::FlagsArray(v) => self.array(FLAGS, v, |w, v| w.put(&v.to_le_bytes()))?,
            Value::ResourceArray(v) => self.array(RESOURCE, v, |w, v| w.guid(*v))?,
            Value::FloatArray(v) => self.array(FLOAT, v, |w, v| w.put(&v.to_le_bytes()))?,
            Value::Vec2Array(v) => self.array(VEC2, v, |w, v| w.f32s(v))?,
            Value::Vec3Array(v) => self.array(VEC3, v, |w, v| w.f32s(v))?,
            Value::Vec4Array(v) => self.array(VEC4, v, |w, v| w.f32s(v))?,
            Value::BoolArray(v) => self.array(BOOL, v, |w, v| w.put(&[u8::from(*v)]))?,
            Value::StringArray(v) => {
                self.array_header(STRING, v.len())?;
                for string in v {
                    self.string(string)?;
                }
            }
        }
        Ok(())
    }

    fn array<T>(
        &mut self,
        tag: u8,
        values: &[T],
        write: impl Fn(&mut Self, &T),
    ) -> Result<(), EntError> {
        self.array_header(tag, values.len())?;
        for value in values {
            write(self, value);
        }
        Ok(())
    }

    fn array_header(&mut self, tag: u8, len: usize) -> Result<(), EntError> {
        let count = u32::try_from(len).map_err(|_| EntError::TooLarge("array"))?;
        self.tag(ARRAY | tag).put(&count.to_le_bytes());
        Ok(())
    }

    fn tag(&mut self, tag: u8) -> &mut Self {
        self.out.push(tag);
        self
    }

    fn put(&mut self, bytes: &[u8]) {
        self.out.extend(bytes);
    }

    fn f32s(&mut self, values: &[f32]) {
        for value in values {
            self.put(&value.to_le_bytes());
        }
    }
}
