use std::{
    collections::{BTreeSet, HashMap, HashSet},
    io::{Cursor, Read, Write},
};

use crate::errors::{AffError, SqfcError};

/// Version 1, [`SqfcInstruction::MakeArray`] sizes are `u16`. What Arma 3 ships.
pub const SQFC_VERSION_1: u32 = 1;
/// Version 2, adds [`SqfcConstant::HashMap`] and [`SqfcInstruction::MakeHashMap`], array sizes
/// are `u32`.
pub const SQFC_VERSION_2: u32 = 2;
/// The only compression of the compressed blocks, LZO1X.
const COMPRESSION_LZO: u8 = 2;
/// [`SqfcCodeSource::Range`] is set in the top bit of the stored `u64`.
const RANGE_FLAG: u64 = 1 << 63;
/// Strings store their length in 3 bytes.
const MAX_STRING_LEN: usize = 0xff_ffff;

const BLOCK_CONSTANTS: u8 = 0;
const BLOCK_CONSTANTS_COMPRESSED: u8 = 1;
const BLOCK_LOCATIONS: u8 = 2;
const BLOCK_CODE: u8 = 3;
const BLOCK_CODE_DEBUG: u8 = 4;
const BLOCK_NAMES: u8 = 5;

/// A compiled script, see the [module docs](super).
#[derive(Debug, Clone, PartialEq)]
pub struct Sqfc {
    /// [`SQFC_VERSION_1`] or [`SQFC_VERSION_2`].
    pub version: u32,
    /// Everything the code pushes, the code blocks themselves and the preprocessed script.
    pub constants: Vec<SqfcConstant>,
    /// The files the script was preprocessed from, [`SqfcLocation::file`] indexes them.
    pub files: Vec<String>,
    /// Index of the [`SqfcConstant::Code`] that runs, see [`Sqfc::code`].
    pub code_index: u64,
    /// The command and variable names, stored once, the instructions refer to them by index.
    /// `None` stores them in each instruction. Arma 3's compiler sorts them, see
    /// [`Sqfc::sort_name_table`], HEMTT keeps the order they appear in. The compilers leave the
    /// table out if there are no names.
    pub name_table: Option<Vec<String>>,
    /// Whether the constants are LZO compressed (what the compiler writes).
    pub compressed: bool,
}

/// A constant of a script.
#[derive(Debug, Clone, PartialEq)]
pub enum SqfcConstant {
    /// A block of code, `{...}`.
    Code(SqfcCode),
    String(String),
    Scalar(f32),
    Boolean(bool),
    Array {
        /// Version 2, could be a read only flag. Version 1 doesn't store it, it is `0` there.
        flags: u8,
        items: Vec<Self>,
    },
    /// A nular command whose result is fixed, like `true` or `objNull`, lower case.
    NularCommand(String),
    /// Version 2.
    HashMap {
        /// Could be a read only flag.
        flags: u8,
        /// Key, value, key, value and so on.
        items: Vec<Self>,
    },
}

/// A block of code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqfcCode {
    /// Where its source text is, see [`Sqfc::code_text`].
    pub source: SqfcCodeSource,
    pub instructions: Vec<SqfcInstruction>,
}

/// Where the source text of a [`SqfcCode`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqfcCodeSource {
    /// A [`SqfcConstant::String`] with the whole preprocessed script, for the code that runs.
    Constant(u64),
    /// Bytes of the preprocessed script, without the braces, for code blocks inside it.
    Range { offset: u32, length: u32 },
}

/// An instruction of the script's stack machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqfcInstruction {
    /// Ends a statement, `;`, clears the stack.
    EndStatement,
    /// Pushes a constant.
    Push { constant: u16 },
    /// Calls a unary command (or a function of the name) with the top of the stack.
    CallUnary {
        name: String,
        location: SqfcLocation,
    },
    /// Calls a binary command with the two values on top of the stack.
    CallBinary {
        name: String,
        location: SqfcLocation,
    },
    /// Calls a nular command.
    CallNular {
        name: String,
        location: SqfcLocation,
    },
    /// `name = value`.
    AssignTo {
        name: String,
        location: SqfcLocation,
    },
    /// `private name = value`.
    AssignToLocal {
        name: String,
        location: SqfcLocation,
    },
    /// Pushes a variable.
    GetVariable {
        name: String,
        location: SqfcLocation,
    },
    /// Makes an array of the top `size` values of the stack.
    MakeArray { size: u32, location: SqfcLocation },
    /// Version 2, makes a hash map of the array on top of the stack.
    MakeHashMap { location: SqfcLocation },
}

/// Where an instruction comes from, for error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SqfcLocation {
    /// Byte offset in the preprocessed script.
    pub offset: u32,
    /// Index into [`Sqfc::files`].
    pub file: u8,
    /// Line in that file, from `1`.
    pub line: u16,
}

impl Sqfc {
    pub fn from<I: Read>(input: &mut I) -> Result<Self, AffError> {
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes)?;
        Ok(Self::parse(&bytes)?)
    }

    fn parse(bytes: &[u8]) -> Result<Self, SqfcError> {
        let mut input = Reader { bytes, pos: 0 };
        let version = input.u32()?;
        if version != SQFC_VERSION_1 && version != SQFC_VERSION_2 {
            return Err(SqfcError::UnsupportedVersion(version));
        }
        let mut context = Context {
            version,
            names: None,
        };
        let mut constants = None;
        let mut compressed = false;
        let mut files = None;
        let mut code_index = None;
        while input.remaining() != 0 {
            let block = input.u8()?;
            match block {
                BLOCK_NAMES if context.names.is_none() && constants.is_none() => {
                    let data = input.decompress()?;
                    let mut names = Reader {
                        bytes: &data,
                        pos: 0,
                    };
                    let count = names.u16()?;
                    let list = (0..count)
                        .map(|_| names.string())
                        .collect::<Result<Vec<_>, _>>()?;
                    names.end()?;
                    context.names = Some(list);
                }
                BLOCK_CONSTANTS | BLOCK_CONSTANTS_COMPRESSED if constants.is_none() => {
                    compressed = block == BLOCK_CONSTANTS_COMPRESSED;
                    constants = Some(if compressed {
                        let data = input.decompress()?;
                        let mut data = Reader {
                            bytes: &data,
                            pos: 0,
                        };
                        let list = data.constants(&context)?;
                        data.end()?;
                        list
                    } else {
                        input.constants(&context)?
                    });
                }
                BLOCK_LOCATIONS if files.is_none() => {
                    let count = input.u16()?;
                    files = Some(
                        (0..count)
                            .map(|_| input.string())
                            .collect::<Result<Vec<_>, _>>()?,
                    );
                }
                BLOCK_CODE if code_index.is_none() => code_index = Some(input.u64()?),
                BLOCK_CODE_DEBUG => return Err(SqfcError::UnsupportedBlock(block)),
                BLOCK_CONSTANTS..=BLOCK_NAMES => return Err(SqfcError::RepeatedBlock(block)),
                _ => return Err(SqfcError::UnknownBlock(block)),
            }
        }
        Ok(Self {
            version,
            constants: constants.ok_or(SqfcError::MissingBlock(BLOCK_CONSTANTS))?,
            files: files.unwrap_or_default(),
            code_index: code_index.ok_or(SqfcError::MissingBlock(BLOCK_CODE))?,
            name_table: context.names,
            compressed,
        })
    }

    /// Writes the script at the current position of `out`, the blocks in the compilers' order.
    /// Names missing from [`Sqfc::name_table`] are added to its end, sorted.
    pub fn write<W: Write>(&self, out: &mut W) -> Result<(), AffError> {
        let mut bytes = Vec::new();
        bytes.extend(self.version.to_le_bytes());
        let names = if let Some(table) = &self.name_table {
            let mut names = table.iter().map(String::as_str).collect::<Vec<_>>();
            let known = names.iter().copied().collect::<HashSet<_>>();
            names.extend(
                self.names()
                    .into_iter()
                    .filter(|name| !known.contains(name)),
            );
            let mut table = Vec::new();
            push_u16(&mut table, names.len(), SqfcError::TooManyNames)?;
            let mut indices = HashMap::new();
            for (index, name) in names.into_iter().enumerate() {
                push_string(&mut table, name)?;
                // Fits, the count did.
                indices.entry(name).or_insert(index as u16);
            }
            bytes.push(BLOCK_NAMES);
            push_compressed(&mut bytes, &table)?;
            Some(indices)
        } else {
            None
        };
        let writer = ConstantWriter {
            version: self.version,
            names: names.as_ref(),
        };
        let mut constants = Vec::new();
        push_u16(
            &mut constants,
            self.constants.len(),
            SqfcError::TooManyConstants,
        )?;
        for constant in &self.constants {
            writer.constant(&mut constants, constant)?;
        }
        if self.compressed {
            bytes.push(BLOCK_CONSTANTS_COMPRESSED);
            push_compressed(&mut bytes, &constants)?;
        } else {
            bytes.push(BLOCK_CONSTANTS);
            bytes.extend(constants);
        }
        bytes.push(BLOCK_LOCATIONS);
        push_u16(&mut bytes, self.files.len(), SqfcError::TooManyFiles)?;
        for file in &self.files {
            push_string(&mut bytes, file)?;
        }
        bytes.push(BLOCK_CODE);
        bytes.extend(self.code_index.to_le_bytes());
        out.write_all(&bytes)?;
        Ok(())
    }

    /// The code that runs.
    #[must_use]
    pub fn code(&self) -> Option<&SqfcCode> {
        match self.constant(self.code_index)? {
            SqfcConstant::Code(code) => Some(code),
            _ => None,
        }
    }

    /// The preprocessed script, the source of [`Sqfc::code`].
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        match self.code()?.source {
            SqfcCodeSource::Constant(index) => match self.constant(index)? {
                SqfcConstant::String(source) => Some(source),
                _ => None,
            },
            SqfcCodeSource::Range { .. } => None,
        }
    }

    /// The source text of a code block, `None` if it is not in the file.
    #[must_use]
    pub fn code_text(&self, code: &SqfcCode) -> Option<&str> {
        match code.source {
            SqfcCodeSource::Constant(index) => match self.constant(index)? {
                SqfcConstant::String(source) => Some(source),
                _ => None,
            },
            SqfcCodeSource::Range { offset, length } => {
                let start = offset as usize;
                self.source()?
                    .get(start..start.checked_add(length as usize)?)
            }
        }
    }

    /// The file an instruction comes from.
    #[must_use]
    pub fn file(&self, location: &SqfcLocation) -> Option<&str> {
        self.files
            .get(usize::from(location.file))
            .map(String::as_str)
    }

    /// Sets [`Sqfc::name_table`] to the sorted names of all instructions, `None` if there are
    /// none. That is what Arma 3's compiler writes.
    pub fn sort_name_table(&mut self) {
        let names = self
            .names()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        self.name_table = (!names.is_empty()).then_some(names);
    }

    fn constant(&self, index: u64) -> Option<&SqfcConstant> {
        self.constants.get(usize::try_from(index).ok()?)
    }

    /// The names of all instructions, the name table.
    fn names(&self) -> BTreeSet<&str> {
        fn collect<'a>(constant: &'a SqfcConstant, names: &mut BTreeSet<&'a str>) {
            match constant {
                SqfcConstant::Code(code) => {
                    names.extend(code.instructions.iter().filter_map(SqfcInstruction::name));
                }
                SqfcConstant::Array { items, .. } | SqfcConstant::HashMap { items, .. } => {
                    for item in items {
                        collect(item, names);
                    }
                }
                _ => {}
            }
        }
        let mut names = BTreeSet::new();
        for constant in &self.constants {
            collect(constant, &mut names);
        }
        names
    }
}

impl SqfcInstruction {
    /// The command or variable name.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::CallUnary { name, .. }
            | Self::CallBinary { name, .. }
            | Self::CallNular { name, .. }
            | Self::AssignTo { name, .. }
            | Self::AssignToLocal { name, .. }
            | Self::GetVariable { name, .. } => Some(name),
            Self::EndStatement
            | Self::Push { .. }
            | Self::MakeArray { .. }
            | Self::MakeHashMap { .. } => None,
        }
    }

    /// Where the instruction comes from, [`SqfcInstruction::EndStatement`] and
    /// [`SqfcInstruction::Push`] don't store it.
    #[must_use]
    pub const fn location(&self) -> Option<&SqfcLocation> {
        match self {
            Self::CallUnary { location, .. }
            | Self::CallBinary { location, .. }
            | Self::CallNular { location, .. }
            | Self::AssignTo { location, .. }
            | Self::AssignToLocal { location, .. }
            | Self::GetVariable { location, .. }
            | Self::MakeArray { location, .. }
            | Self::MakeHashMap { location } => Some(location),
            Self::EndStatement | Self::Push { .. } => None,
        }
    }

    const fn opcode(&self) -> u8 {
        match self {
            Self::EndStatement => 0,
            Self::Push { .. } => 1,
            Self::CallUnary { .. } => 2,
            Self::CallBinary { .. } => 3,
            Self::CallNular { .. } => 4,
            Self::AssignTo { .. } => 5,
            Self::AssignToLocal { .. } => 6,
            Self::GetVariable { .. } => 7,
            Self::MakeArray { .. } => 8,
            Self::MakeHashMap { .. } => 9,
        }
    }
}

impl SqfcCodeSource {
    const fn from_raw(raw: u64) -> Self {
        if raw & RANGE_FLAG == 0 {
            Self::Constant(raw)
        } else {
            Self::Range {
                offset: raw as u32,
                length: ((raw >> 32) & 0x7fff_ffff) as u32,
            }
        }
    }

    fn to_raw(self) -> Result<u64, SqfcError> {
        match self {
            Self::Constant(index) if index & RANGE_FLAG == 0 => Ok(index),
            Self::Range { offset, length } if length <= 0x7fff_ffff => {
                Ok(RANGE_FLAG | u64::from(length) << 32 | u64::from(offset))
            }
            _ => Err(SqfcError::InvalidSource),
        }
    }
}

/// What reading instructions needs.
struct Context {
    version: u32,
    names: Option<Vec<String>>,
}

struct ConstantWriter<'a> {
    version: u32,
    names: Option<&'a HashMap<&'a str, u16>>,
}

impl ConstantWriter<'_> {
    fn constant(&self, out: &mut Vec<u8>, constant: &SqfcConstant) -> Result<(), SqfcError> {
        match constant {
            SqfcConstant::Code(code) => {
                out.push(0);
                out.extend(code.source.to_raw()?.to_le_bytes());
                let count = u32::try_from(code.instructions.len())
                    .map_err(|_| SqfcError::TooManyInstructions)?;
                out.extend(count.to_le_bytes());
                for instruction in &code.instructions {
                    self.instruction(out, instruction)?;
                }
            }
            SqfcConstant::String(string) => {
                out.push(1);
                push_string(out, string)?;
            }
            SqfcConstant::Scalar(value) => {
                out.push(2);
                out.extend(value.to_le_bytes());
            }
            SqfcConstant::Boolean(value) => out.extend([3, u8::from(*value)]),
            SqfcConstant::Array { flags, items } | SqfcConstant::HashMap { flags, items } => {
                let hash_map = matches!(constant, SqfcConstant::HashMap { .. });
                if hash_map && (self.version < SQFC_VERSION_2 || !items.len().is_multiple_of(2)) {
                    return Err(SqfcError::InvalidHashMap);
                }
                out.push(if hash_map { 6 } else { 4 });
                let count = u32::try_from(items.len()).map_err(|_| SqfcError::TooManyConstants)?;
                out.extend(count.to_le_bytes());
                if self.version >= SQFC_VERSION_2 {
                    out.push(*flags);
                } else if *flags != 0 {
                    return Err(SqfcError::InvalidFlags(*flags));
                }
                for item in items {
                    self.constant(out, item)?;
                }
            }
            SqfcConstant::NularCommand(name) => {
                out.push(5);
                push_string(out, name)?;
            }
        }
        Ok(())
    }

    fn instruction(
        &self,
        out: &mut Vec<u8>,
        instruction: &SqfcInstruction,
    ) -> Result<(), SqfcError> {
        out.push(instruction.opcode());
        if let Some(location) = instruction.location() {
            out.extend(location.offset.to_le_bytes());
            out.push(location.file);
            out.extend(location.line.to_le_bytes());
        }
        match instruction {
            SqfcInstruction::EndStatement => {}
            SqfcInstruction::Push { constant } => out.extend(constant.to_le_bytes()),
            SqfcInstruction::MakeArray { size, .. } => {
                if self.version == SQFC_VERSION_1 {
                    let size = u16::try_from(*size).map_err(|_| SqfcError::ArrayTooLarge(*size))?;
                    out.extend(size.to_le_bytes());
                } else {
                    out.extend(size.to_le_bytes());
                }
            }
            SqfcInstruction::MakeHashMap { .. } => {
                if self.version < SQFC_VERSION_2 {
                    return Err(SqfcError::InvalidHashMap);
                }
            }
            SqfcInstruction::CallUnary { name, .. }
            | SqfcInstruction::CallBinary { name, .. }
            | SqfcInstruction::CallNular { name, .. }
            | SqfcInstruction::AssignTo { name, .. }
            | SqfcInstruction::AssignToLocal { name, .. }
            | SqfcInstruction::GetVariable { name, .. } => match self.names {
                // `Sqfc::write` added every name.
                Some(names) => out.extend(
                    names
                        .get(name.as_str())
                        .copied()
                        .unwrap_or_default()
                        .to_le_bytes(),
                ),
                None => push_string(out, name)?,
            },
        }
        Ok(())
    }
}

fn push_u16(out: &mut Vec<u8>, value: usize, error: SqfcError) -> Result<(), SqfcError> {
    out.extend(u16::try_from(value).map_err(|_| error)?.to_le_bytes());
    Ok(())
}

fn push_string(out: &mut Vec<u8>, string: &str) -> Result<(), SqfcError> {
    let len = string.len();
    if len > MAX_STRING_LEN {
        return Err(SqfcError::StringTooLong(len));
    }
    out.extend(&(len as u32).to_le_bytes()[..3]);
    out.extend(string.as_bytes());
    Ok(())
}

/// The uncompressed size, the compression and the LZO stream.
fn push_compressed(out: &mut Vec<u8>, data: &[u8]) -> Result<(), SqfcError> {
    let size = u32::try_from(data.len()).map_err(|_| SqfcError::TooLarge)?;
    out.extend(size.to_le_bytes());
    out.push(COMPRESSION_LZO);
    out.extend(lzokay_native::compress(data).map_err(|e| SqfcError::Lzo(e.to_string()))?);
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    const fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    const fn end(&self) -> Result<(), SqfcError> {
        match self.remaining() {
            0 => Ok(()),
            n => Err(SqfcError::TrailingData(n)),
        }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], SqfcError> {
        let bytes = self
            .pos
            .checked_add(len)
            .and_then(|end| self.bytes.get(self.pos..end))
            .ok_or(SqfcError::Truncated)?;
        self.pos += len;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], SqfcError> {
        Ok(self.take(N)?.try_into().unwrap_or([0; N]))
    }

    fn u8(&mut self) -> Result<u8, SqfcError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, SqfcError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, SqfcError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, SqfcError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn string(&mut self) -> Result<String, SqfcError> {
        let [a, b, c] = self.array()?;
        let len = u32::from_le_bytes([a, b, c, 0]) as usize;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| SqfcError::InvalidString)
    }

    /// A compressed block: the uncompressed size, the compression and the LZO stream, which
    /// ends with its end marker.
    fn decompress(&mut self) -> Result<Vec<u8>, SqfcError> {
        let size = self.u32()? as usize;
        let compression = self.u8()?;
        if compression != COMPRESSION_LZO {
            return Err(SqfcError::UnknownCompression(compression));
        }
        let mut stream = Cursor::new(&self.bytes[self.pos..]);
        let data = lzokay_native::decompress_bounded(&mut stream, size)
            .map_err(|e| SqfcError::Lzo(e.to_string()))?;
        if data.len() != size {
            return Err(SqfcError::Lzo(format!(
                "decompressed to {} bytes, expected {size}",
                data.len()
            )));
        }
        self.pos += stream.position() as usize;
        Ok(data)
    }

    fn constants(&mut self, context: &Context) -> Result<Vec<SqfcConstant>, SqfcError> {
        let count = self.u16()?;
        (0..count).map(|_| self.constant(context)).collect()
    }

    fn constant(&mut self, context: &Context) -> Result<SqfcConstant, SqfcError> {
        Ok(match self.u8()? {
            0 => {
                let source = SqfcCodeSource::from_raw(self.u64()?);
                let count = self.u32()? as usize;
                // Each instruction has at least a byte.
                let mut instructions = Vec::with_capacity(count.min(self.remaining()));
                for _ in 0..count {
                    instructions.push(self.instruction(context)?);
                }
                SqfcConstant::Code(SqfcCode {
                    source,
                    instructions,
                })
            }
            1 => SqfcConstant::String(self.string()?),
            2 => SqfcConstant::Scalar(f32::from_le_bytes(self.array()?)),
            3 => match self.u8()? {
                0 => SqfcConstant::Boolean(false),
                1 => SqfcConstant::Boolean(true),
                value => return Err(SqfcError::InvalidBoolean(value)),
            },
            kind @ (4 | 6) => {
                let count = self.u32()? as usize;
                let flags = if context.version >= SQFC_VERSION_2 {
                    self.u8()?
                } else {
                    0
                };
                if kind == 6 && (context.version < SQFC_VERSION_2 || !count.is_multiple_of(2)) {
                    return Err(SqfcError::InvalidHashMap);
                }
                let mut items = Vec::with_capacity(count.min(self.remaining()));
                for _ in 0..count {
                    items.push(self.constant(context)?);
                }
                if kind == 4 {
                    SqfcConstant::Array { flags, items }
                } else {
                    SqfcConstant::HashMap { flags, items }
                }
            }
            5 => SqfcConstant::NularCommand(self.string()?),
            kind => return Err(SqfcError::UnknownConstant(kind)),
        })
    }

    fn instruction(&mut self, context: &Context) -> Result<SqfcInstruction, SqfcError> {
        let opcode = self.u8()?;
        if opcode <= 1 {
            return Ok(if opcode == 0 {
                SqfcInstruction::EndStatement
            } else {
                SqfcInstruction::Push {
                    constant: self.u16()?,
                }
            });
        }
        if opcode > 9 || (opcode == 9 && context.version < SQFC_VERSION_2) {
            return Err(SqfcError::UnknownInstruction(opcode));
        }
        let location = SqfcLocation {
            offset: self.u32()?,
            file: self.u8()?,
            line: self.u16()?,
        };
        Ok(match opcode {
            8 => SqfcInstruction::MakeArray {
                size: if context.version == SQFC_VERSION_1 {
                    u32::from(self.u16()?)
                } else {
                    self.u32()?
                },
                location,
            },
            9 => SqfcInstruction::MakeHashMap { location },
            _ => {
                let name = match &context.names {
                    Some(names) => {
                        let index = self.u16()?;
                        names
                            .get(usize::from(index))
                            .ok_or(SqfcError::InvalidName(index))?
                            .clone()
                    }
                    None => self.string()?,
                };
                match opcode {
                    2 => SqfcInstruction::CallUnary { name, location },
                    3 => SqfcInstruction::CallBinary { name, location },
                    4 => SqfcInstruction::CallNular { name, location },
                    5 => SqfcInstruction::AssignTo { name, location },
                    6 => SqfcInstruction::AssignToLocal { name, location },
                    _ => SqfcInstruction::GetVariable { name, location },
                }
            }
        })
    }
}
