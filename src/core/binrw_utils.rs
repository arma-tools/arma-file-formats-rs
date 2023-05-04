use std::io::Cursor;
use std::io::Read;
use std::io::Seek;

use binrw::BinRead;
use binrw::BinResult;
use binrw::Endian;
use derivative::Derivative;
use lzokay_rust_native::decompress::decompress_stream;
use rsa::BigUint;

use crate::p3d::ODOLArgs;

use super::decompress_lzss;
use super::types::STPair;
use super::types::XYZTripletBinrw;

// #[binrw::parser(reader, endian)]
// fn read_lzo(expected_size: usize) -> BinResult<Vec<u8>> {
//     let pre_pos = reader.stream_position()?;
//     let mut buf_reader = BufReader::new(reader);
//     decompress_stream(&mut buf_reader, Some(expected_size)).map_err(|e| binrw::Error::Custom {
//         err: Box::new(e),
//         pos: pre_pos,
//     })
// }

#[binrw::parser(reader, endian)]
pub(crate) fn read_compressed_size_cond(
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

// #[binrw::parser(reader, endian)]
// pub(crate) fn read_compressed_size(
//     elemen_size: usize,
//     count: usize,
//     args: ODOLArgs,
// ) -> BinResult<Vec<u8>> {
//     decompress_data(reader, endian, elemen_size, count, args)
// }

#[binrw::parser(reader, endian)]
pub(crate) fn read_compressed(elemen_size: usize, args: ODOLArgs) -> BinResult<Vec<u8>> {
    let count = u32::read_options(reader, endian, ())? as usize;
    decompress_data(reader, endian, elemen_size, count, args)
}

#[binrw::parser(reader, endian)]
pub(crate) fn read_compressed_array<T, 'a>(
    elemen_size: usize,
    odol_args: ODOLArgs,
) -> BinResult<Vec<T>>
where
    T: BinRead<Args<'a> = ()>,
{
    let count = u32::read_options(reader, endian, ())? as usize;
    decompress_array(reader, endian, elemen_size, count, odol_args)
}

#[binrw::parser(reader, endian)]
pub(crate) fn read_compressed_array_count<T, 'a>(
    elemen_size: usize,
    count: usize,
    odol_args: ODOLArgs,
) -> BinResult<Vec<T>>
where
    T: BinRead<Args<'a> = ()>,
{
    decompress_array(reader, endian, elemen_size, count, odol_args)
}

pub(crate) fn decompress_array<'a, T>(
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
pub(crate) fn read_compressed_data_cond_count(
    condition: bool,
    count: usize,
    odol_args: ODOLArgs,
) -> BinResult<Option<Vec<u8>>> {
    // dbg!(count);
    // dbg!(odol_args);
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
    let data = if odol_args.use_lzo {
        let mut flag = expected_size > 1024;
        if odol_args.use_compression_flag {
            flag = u8::read_options(reader, endian, ())? != 0;
        }

        if !flag {
            let mut data = vec![0; expected_size];
            reader
                .read_exact(&mut data)
                .map_err(|e| binrw::Error::Custom {
                    err: Box::new(e),
                    pos: pre_pos,
                })?;
            data
        } else {
            decompress_stream(reader, Some(count * elemen_size)).map_err(|e| {
                binrw::Error::Custom {
                    err: Box::new(e),
                    pos: pre_pos,
                }
            })?
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
pub(crate) fn read_condensed_array_cond<T, 'a>(
    cond: bool,
    elemen_size: usize,
    args: ODOLArgs,
) -> BinResult<Option<Vec<T>>>
where
    T: BinRead<Args<'a> = ()> + Clone,
    // T::Args<'a>: Clone,
    // T: Clone,
{
    if cond {
        Ok(Some(condensed_array(reader, endian, elemen_size, args)?))
    } else {
        Ok(None)
    }
}

// #[binrw::parser(reader, endian)]
// pub(crate) fn read_condensed_array<T, 'a>(elemen_size: usize, args: ODOLArgs) -> BinResult<Vec<T>>
// where
//     T: BinRead<Args<'a> = ()> + Clone,
//     // T::Args<'a>: Clone,
//     // T: Clone,
// {
//     condensed_array(reader, endian, elemen_size, args)
// }

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
pub(crate) fn read_vertex_index_array(args: ODOLArgs, count: usize) -> BinResult<Vec<u32>> {
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
        Ok(u16::read_options(reader, endian, ())? as u32)
    }
}

#[binrw::parser(reader, endian)]
pub(crate) fn read_normals_parse(args: (ODOLArgs,)) -> BinResult<Vec<XYZTripletBinrw>> {
    read_normals(reader, endian, args.0)
}

pub(crate) fn read_normals(
    reader: &mut (impl Read + Seek),
    endian: Endian,
    args: ODOLArgs,
) -> BinResult<Vec<XYZTripletBinrw>> {
    if args.version >= 45 {
        let comp = condensed_array::<i32>(reader, endian, 4, args)?;
        Ok(comp.into_iter().map(decompress_xyz).collect())
    } else {
        Ok(condensed_array::<XYZTripletBinrw>(
            reader, endian, 12, args,
        )?)
    }
}

pub(crate) fn decompress_xyz(val: i32) -> XYZTripletBinrw {
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

    XYZTripletBinrw {
        x: x as f32 * factor,
        y: y as f32 * factor,
        z: z as f32 * factor,
    }
}

#[binrw::parser(reader, endian)]
pub(crate) fn read_st_parse(args: (ODOLArgs,)) -> BinResult<Vec<STPair>> {
    read_st(reader, endian, args.0)
}

pub(crate) fn read_st(
    reader: &mut (impl Read + Seek),
    endian: Endian,
    args: ODOLArgs,
) -> BinResult<Vec<STPair>> {
    let count = u32::read_options(reader, endian, ())? as usize;
    if args.version >= 45 {
        let comp = decompress_array::<STPairCompress>(reader, endian, 8, count, args)?;
        Ok(comp.into_iter().map(|x| x.into()).collect())
    } else {
        Ok(decompress_array::<STPair>(reader, endian, 24, count, args)?)
    }
}

#[derive(PartialEq, BinRead, Derivative, Clone, Copy)]
#[derivative(Debug, Default)]
struct STPairCompress {
    s: i32,
    t: i32,
}

impl From<STPairCompress> for STPair {
    fn from(val: STPairCompress) -> Self {
        STPair {
            s: decompress_xyz(val.s),
            t: decompress_xyz(val.t),
        }
    }
}

#[binrw::parser(reader)]
pub(crate) fn read_biguint(length: (usize,)) -> BinResult<BigUint> {
    let mut buf = vec![0_u8; length.0];
    reader.read_exact(&mut buf)?;
    Ok(BigUint::from_bytes_le(&buf))
}

#[binrw::writer(writer)]
pub(crate) fn write_biguint(biguint: &BigUint) -> BinResult<()> {
    let buf = biguint.to_bytes_le();
    writer.write_all(&buf)?;
    Ok(())
}

// pub(crate) fn read_biguint(
//     rest: &BitSlice<u8, Msb0>,
//     length: usize,
// ) -> Result<(&BitSlice<u8, Msb0>, BigUint), DekuError> {
//     let (bigint_bytes, rest) = rest.split_at(length * 8);
//     Ok((
//         rest,
//         BigUint::from_bytes_le(&bigint_bytes.to_bitvec().into_vec()),
//     ))
// }

// pub(crate) fn write_biguint(
//     output: &mut BitVec<u8, Msb0>,
//     bigint: &BigUint,
// ) -> Result<(), DekuError> {
//     let bigint_bytes = bigint.to_bytes_le();
//     output.write_all(&bigint_bytes).unwrap();
//     Ok(())
// }
