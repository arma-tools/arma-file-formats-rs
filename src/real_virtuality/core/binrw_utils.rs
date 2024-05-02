use std::io::Cursor;
use std::io::Read;
use std::io::Seek;

use binrw::BinRead;
use binrw::BinResult;
use binrw::Endian;
use lzokay_native::decompress;
use rsa::BigUint;

use crate::real_virtuality::p3d::ODOLArgs;

use super::decompress_lzss;
use super::types::STPair;
use super::types::XYZTriplet;

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

    let mut reader2 = Cursor::new(data);
    let mut arr = Vec::with_capacity(count);

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
    let expected_size = count * elemen_size;
    if expected_size == 0 {
        return Ok(Vec::new());
    }
    let pre_pos = reader.stream_position()?;
    //dbg!(pre_pos);
    let data = if odol_args.use_lzo {
        let flag = if odol_args.use_compression_flag {
            u8::read_options(reader, endian, ())? != 0
        } else {
            expected_size >= 1024
        };

        if flag {
            decompress(reader, Some(count * elemen_size)).map_err(|e| binrw::Error::Custom {
                err: Box::new(e),
                pos: pre_pos,
            })?
        } else {
            let mut data = vec![0; expected_size];
            reader
                .read_exact(&mut data)
                .map_err(|e| binrw::Error::Custom {
                    err: Box::new(e),
                    pos: pre_pos,
                })?;
            data
        }
    } else if expected_size < 1024 {
        let mut data = vec![0; expected_size];
        reader
            .read_exact(&mut data)
            .map_err(|e| binrw::Error::Custom {
                err: Box::new(e),
                pos: pre_pos,
            })?;
        data
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
        let val = T::read_options(reader, endian, ())?;
        vec![val; count]
    } else {
        decompress_array(reader, endian, elemen_size, count, args)?
    };

    Ok(res)
}

#[binrw::parser(reader, endian)]
pub fn read_vertex_index_array(args: ODOLArgs, count: usize) -> BinResult<Vec<u32>> {
    let mut res = Vec::with_capacity(count);

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

#[binrw::parser(reader)]
pub fn read_biguint(length: usize) -> BinResult<BigUint> {
    let mut buf = vec![0_u8; length];
    reader.read_exact(&mut buf)?;
    Ok(BigUint::from_bytes_le(&buf))
}

#[binrw::writer(writer)]
pub fn write_biguint(biguint: &BigUint) -> BinResult<()> {
    let buf = biguint.to_bytes_le();
    writer.write_all(&buf)?;
    Ok(())
}
