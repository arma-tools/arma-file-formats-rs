use binrw::BinResult;
use rsa::BigUint;

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
