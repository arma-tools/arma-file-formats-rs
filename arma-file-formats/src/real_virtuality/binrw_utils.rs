use std::io::{Cursor, Read, Seek};

use binrw::{BinRead, BinResult, Endian};

use crate::{core::decompress_lzss, real_virtuality::p3d::ODOLArgs};

use super::types::{STPair, XYZTriplet};

#[binrw::parser(reader, endian)]
pub fn read_compressed_size_cond(
    condition: bool,
    elemen_size: usize,
    count: usize,
    args: ODOLArgs,
) -> BinResult<Option<Vec<u8>>> {
    if condition {
        Ok(Some(decompress_data(
            reader,
            endian,
            elemen_size,
            count,
            args,
        )?))
    } else {
        Ok(None)
    }
}

#[binrw::parser(reader, endian)]
pub fn read_compressed(elemen_size: usize, args: ODOLArgs) -> BinResult<Vec<u8>> {
    let count = u32::read_options(reader, endian, ())? as usize;
    decompress_data(reader, endian, elemen_size, count, args)
}

#[binrw::parser(reader, endian)]
pub fn read_compressed_array<T, 'a>(elemen_size: usize, odol_args: ODOLArgs) -> BinResult<Vec<T>>
where
    T: BinRead<Args<'a> = ()>,
{
    let count = u32::read_options(reader, endian, ())? as usize;
    decompress_array(reader, endian, elemen_size, count, odol_args)
}

#[binrw::parser(reader, endian)]
pub fn read_compressed_array_count<T, 'a>(
    elemen_size: usize,
    count: usize,
    odol_args: ODOLArgs,
) -> BinResult<Vec<T>>
where
    T: BinRead<Args<'a> = ()>,
{
    decompress_array(reader, endian, elemen_size, count, odol_args)
}

pub fn decompress_array<'a, T>(
    reader: &mut (impl Read + Seek),
    endian: Endian,
    elemen_size: usize,
    count: usize,
    odol_args: ODOLArgs,
) -> BinResult<Vec<T>>
where
    T: BinRead<Args<'a> = ()>,
{
    let data = decompress_data(reader, endian, elemen_size, count, odol_args)?;

    let mut reader2 = Cursor::new(&data);
    let mut arr = Vec::with_capacity(count.min(data.len() / elemen_size.max(1)));

    for _ in 0..count {
        let el = T::read_options(&mut reader2, endian, ())?;
        arr.push(el);
    }
    Ok(arr)
}

#[binrw::parser(reader, endian)]
pub fn read_compressed_data_cond_count(
    condition: bool,
    count: usize,
    odol_args: ODOLArgs,
) -> BinResult<Option<Vec<u8>>> {
    if condition {
        Ok(Some(decompress_data(reader, endian, 1, count, odol_args)?))
    } else {
        Ok(None)
    }
}
fn decompress_data(
    reader: &mut (impl Read + Seek),
    endian: Endian,
    elemen_size: usize,
    count: usize,
    odol_args: ODOLArgs,
) -> BinResult<Vec<u8>> {
    let pre_pos = reader.stream_position()?;
    let expected_size = count
        .checked_mul(elemen_size)
        .ok_or_else(|| binrw::Error::AssertFail {
            pos: pre_pos,
            message: format!("Array of {count} elements of {elemen_size} bytes is too large"),
        })?;
    if expected_size == 0 {
        return Ok(Vec::new());
    }
    let data = if odol_args.use_lzo {
        let flag = if odol_args.use_compression_flag {
            u8::read_options(reader, endian, ())? != 0
        } else {
            expected_size >= 1024
        };

        if flag {
            lzokay_native::decompress_bounded(reader, expected_size).map_err(|e| {
                binrw::Error::Custom {
                    err: Box::new(e),
                    pos: pre_pos,
                }
            })?
        } else {
            read_bytes(reader, expected_size, pre_pos)?
        }
    } else if expected_size < 1024 {
        read_bytes(reader, expected_size, pre_pos)?
    } else {
        decompress_lzss(reader, expected_size, false)
            .map_err(|e| binrw::Error::Custom {
                err: Box::new(e),
                pos: pre_pos,
            })?
            .1
    };
    Ok(data)
}

/// Reads `size` bytes, growing the buffer with the data actually there: `size` can be garbage.
fn read_bytes(reader: &mut impl Read, size: usize, pos: u64) -> BinResult<Vec<u8>> {
    let mut data = Vec::with_capacity(size.min(1 << 20));
    let read = reader
        .take(size as u64)
        .read_to_end(&mut data)
        .map_err(|e| binrw::Error::Custom {
            err: Box::new(e),
            pos,
        })?;
    if read != size {
        return Err(binrw::Error::Io(std::io::ErrorKind::UnexpectedEof.into()));
    }
    Ok(data)
}

/// Most elements a default-filled condensed array may have. These arrays hold one value per
/// vertex, the largest LOD of Arma 3 and ~28,000 mod models has 669,004 (an NH90 helicopter).
/// Garbage counts would otherwise allocate up to 48 GB from a single value.
const MAX_DEFAULT_FILL: usize = 1 << 24;

#[binrw::parser(reader, endian)]
pub fn read_condensed_array_cond<T, 'a>(
    cond: bool,
    elemen_size: usize,
    args: ODOLArgs,
) -> BinResult<Option<Vec<T>>>
where
    T: BinRead<Args<'a> = ()> + Clone,
{
    if cond {
        Ok(Some(condensed_array(reader, endian, elemen_size, args)?))
    } else {
        Ok(None)
    }
}

fn condensed_array<'a, T>(
    reader: &mut (impl Read + Seek),
    endian: Endian,
    elemen_size: usize,
    args: ODOLArgs,
) -> Result<Vec<T>, binrw::Error>
where
    T: BinRead<Args<'a> = ()> + Clone,
{
    let count = u32::read_options(reader, endian, ())? as usize;

    let default_fill = u8::read_options(reader, endian, ())? != 0;

    let res: Vec<T> = if default_fill {
        let pos = reader.stream_position()?;
        let val = T::read_options(reader, endian, ())?;
        if count > MAX_DEFAULT_FILL {
            return Err(binrw::Error::AssertFail {
                pos,
                message: format!("Default-filled array of {count} elements"),
            });
        }
        vec![val; count]
    } else {
        decompress_array(reader, endian, elemen_size, count, args)?
    };

    Ok(res)
}

#[binrw::parser(reader, endian)]
pub fn read_vertex_index_array(args: ODOLArgs, count: usize) -> BinResult<Vec<u32>> {
    let mut res = Vec::with_capacity(count.min(1 << 16));

    for _ in 0..count {
        res.push(read_vertex_index(reader, endian, args)?);
    }

    Ok(res)
}

fn read_vertex_index(
    reader: &mut (impl Read + Seek),
    endian: Endian,
    args: ODOLArgs,
) -> BinResult<u32> {
    if args.version >= 69 {
        u32::read_options(reader, endian, ())
    } else {
        Ok(u32::from(u16::read_options(reader, endian, ())?))
    }
}

#[binrw::parser(reader, endian)]
pub fn read_normals_parse(args: ODOLArgs) -> BinResult<Vec<XYZTriplet>> {
    read_normals(reader, endian, args)
}

pub fn read_normals(
    reader: &mut (impl Read + Seek),
    endian: Endian,
    args: ODOLArgs,
) -> BinResult<Vec<XYZTriplet>> {
    if args.version >= 45 {
        let comp = condensed_array::<i32>(reader, endian, 4, args)?;
        Ok(comp.into_iter().map(decompress_xyz).collect())
    } else {
        Ok(condensed_array::<XYZTriplet>(reader, endian, 12, args)?)
    }
}

pub fn decompress_xyz(val: i32) -> XYZTriplet {
    let mut x = val & 1023;
    let mut y = val >> 10 & 1023;
    let mut z = val >> 20 & 1023;

    if x > 511 {
        x -= 1024;
    }

    if y > 511 {
        y -= 1024;
    }

    if z > 511 {
        z -= 1024;
    }

    let factor = -0.001_956_947_1_f32;

    XYZTriplet {
        x: x as f32 * factor,
        y: y as f32 * factor,
        z: z as f32 * factor,
    }
}

#[binrw::parser(reader, endian)]
pub fn read_st_parse(args: ODOLArgs) -> BinResult<Vec<STPair>> {
    read_st(reader, endian, args)
}

pub fn read_st(
    reader: &mut (impl Read + Seek),
    endian: Endian,
    args: ODOLArgs,
) -> BinResult<Vec<STPair>> {
    let count = u32::read_options(reader, endian, ())? as usize;
    if args.version >= 45 {
        let comp = decompress_array::<STPairCompress>(reader, endian, 8, count, args)?;
        Ok(comp.into_iter().map(Into::into).collect())
    } else {
        Ok(decompress_array::<STPair>(reader, endian, 24, count, args)?)
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, BinRead)]
struct STPairCompress {
    s: i32,
    t: i32,
}

impl From<STPairCompress> for STPair {
    fn from(val: STPairCompress) -> Self {
        Self {
            s: decompress_xyz(val.s),
            t: decompress_xyz(val.t),
        }
    }
}

pub fn read_8wvr_material_names(
    reader: &mut (impl Read + Seek),
    endian: Endian,
    material_count: u32,
) -> BinResult<Vec<String>> {
    let mut materials = Vec::new();
    for _ in 0..material_count {
        let mut length;
        loop {
            length = u32::read_options(reader, endian, ())?;

            if length == 0 {
                break;
            }
            let mut data = vec![0; length as usize];
            reader.read_exact(&mut data)?;
            materials.push(String::from_utf8_lossy(&data).to_string());
        }
    }

    Ok(materials)
}
