use std::io::{Cursor, Write};

use byteorder::WriteBytesExt;
use deku::{
    bitvec::{BitSlice, BitVec, Msb0},
    DekuError, DekuRead,
};
use lzokay_rust_native::{compress::compress_worst_size, decompress::decompress_stream};
use rsa::BigUint;

use super::types::BytesUntilZeroData;

pub(crate) fn read_string_zt(
    rest: &BitSlice<u8, Msb0>,
) -> Result<(&BitSlice<u8, Msb0>, String), DekuError> {
    let (rest, val) = BytesUntilZeroData::read(rest, ())?;
    Ok((
        rest,
        String::from_utf8(val.bytes[0..val.bytes.len() - 1].to_vec()).unwrap_or_default(),
    ))
}

pub(crate) fn write_string_zt(output: &mut BitVec<u8, Msb0>, str: &str) -> Result<(), DekuError> {
    let value = str.as_bytes();
    output.write_all(value).unwrap();
    output.write_u8(b'\0').unwrap();
    Ok(())
}

pub(crate) fn read_string_zt_vec(
    mut rest: &BitSlice<u8, Msb0>,
    count: usize,
) -> Result<(&BitSlice<u8, Msb0>, Vec<String>), DekuError> {
    let mut strings = Vec::with_capacity(count);
    for _ in 0..count {
        let (res, val) = read_string_zt(rest)?;
        rest = res;
        strings.push(val);
    }
    Ok((rest, strings))
}

pub(crate) fn write_string_zt_vec(
    output: &mut BitVec<u8, Msb0>,
    vec: &Vec<String>,
) -> Result<(), DekuError> {
    for str in vec {
        write_string_zt(output, str)?;
    }
    Ok(())
}

pub(crate) fn read_lzo(
    map_size: u32,
    rest: &BitSlice<u8, Msb0>,
) -> Result<(&BitSlice<u8, Msb0>, Vec<u8>), DekuError> {
    let worst_compress_size = compress_worst_size(map_size as usize);
    let (left, _) = rest.split_at(worst_compress_size * 8);

    let compressed_data = left.to_bitvec().into_vec();
    let mut reader = Cursor::new(compressed_data);

    let uncompressed_data = decompress_stream(&mut reader, Some(map_size as usize)).unwrap();

    let (_, right) = rest.split_at(reader.position() as usize * 8);

    Ok((right, uncompressed_data))
}

// pub(crate) fn read_string_kl(
//     rest: &BitSlice<u8, Msb0>,
//     len: usize,
// ) -> Result<(&BitSlice<u8, Msb0>, String), DekuError> {
//     let (str_bytes, rest) = rest.split_at(len * 8);
//     Ok((
//         rest,
//         String::from_utf8(str_bytes.to_bitvec().into_vec()).unwrap_or_default(),
//     ))
// }

// pub(crate) fn write_string_kl(output: &mut BitVec<u8, Msb0>, str: &str) -> Result<(), DekuError> {
//     let value = str.as_bytes();
//     output.write_all(value).unwrap();
//     Ok(())
// }

pub(crate) fn read_biguint(
    rest: &BitSlice<u8, Msb0>,
    length: usize,
) -> Result<(&BitSlice<u8, Msb0>, BigUint), DekuError> {
    let (bigint_bytes, rest) = rest.split_at(length * 8);
    Ok((
        rest,
        BigUint::from_bytes_le(&bigint_bytes.to_bitvec().into_vec()),
    ))
}

pub(crate) fn write_biguint(
    output: &mut BitVec<u8, Msb0>,
    bigint: &BigUint,
) -> Result<(), DekuError> {
    let bigint_bytes = bigint.to_bytes_le();
    output.write_all(&bigint_bytes).unwrap();
    Ok(())
}

//BigUint::from_bytes_le
