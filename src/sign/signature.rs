use std::fs::File;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::path::Path;

use crate::core::binrw_utils::{read_biguint, write_biguint};
use crate::core::write::WriteExtTrait;
use crate::errors::RvffError;
use binrw::{binrw, BinRead, Endian};
use binrw::{BinWrite, NullString};
use derivative::Derivative;
use rsa::BigUint;

const EXTENSION: &str = "bisign";

#[derive(Eq, PartialEq, Debug)]
#[binrw]
#[brw(little)]
pub struct Signature {
    pub authority: NullString,

    #[br(assert(unk1 == 148))]
    #[bw(assert(unk1 == &148))]
    unk1: u32,

    #[br(assert(unk2 == 518))]
    #[bw(assert(unk2 == &518))]
    unk2: u32,
    #[br(assert(unk3 == 9216))]
    #[bw(assert(unk3 == &9216))]
    unk3: u32,

    #[br(assert(unk4 == 826_364_754))]
    #[bw(assert(unk4 == &826_364_754))]
    unk4: u32,

    pub(crate) n_length: u32,
    pub exponent: u32,

    #[br(args(n_length as usize / 8))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub n: BigUint,

    pub(crate) sig1_length: u32,

    #[br(args(sig1_length as usize))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub sig1: BigUint,

    pub version: SignVersion,

    pub(crate) sig2_length: u32,

    #[br(args(sig2_length as usize))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub sig2: BigUint,

    pub(crate) sig3_length: u32,

    #[br(args(sig3_length as usize))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub sig3: BigUint,
}

#[allow(non_camel_case_types, clippy::enum_variant_names)]
#[derive(BinRead, BinWrite, Derivative, Eq, PartialEq, Clone, Copy)]
#[derivative(Debug)]
#[brw(repr = u32)]
pub enum SignVersion {
    V2 = 2,
    V3 = 3,
}

impl Signature {
    #[must_use]
    pub fn new() -> Self {
        Self {
            authority: String::default().into(),
            unk1: 148,
            unk2: 518,
            unk3: 9216,
            unk4: 826_364_754,
            n_length: 0,
            exponent: 0,
            n: BigUint::default(),
            sig1_length: 0,
            sig1: BigUint::default(),
            version: SignVersion::V2,
            sig2_length: 0,
            sig2: BigUint::default(),
            sig3_length: 0,
            sig3: BigUint::default(),
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, RvffError> {
        let file = File::open(path)?;
        let mut buf_reader = BufReader::new(file);
        Self::from_stream(&mut buf_reader)
    }

    pub fn from_stream<R>(reader: &mut R) -> Result<Self, RvffError>
    where
        R: Read + Seek,
    {
        let sig = Self::read_options(reader, Endian::Little, ())?;
        Ok(sig)
    }

    pub fn write_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), RvffError> {
        let path: &Path = &path.as_ref().with_extension(EXTENSION);

        let mut file = File::create(path)?;
        file.write_bytes(&self.write_data()?)?;
        Ok(())
    }

    pub fn write_data(&mut self) -> Result<Vec<u8>, RvffError> {
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);

        self.n_length = (self.n.to_bytes_le().len() * 8) as u32;

        Self::write(self, &mut cursor)?;

        Ok(buf)
    }

    pub(crate) fn get_hashes(&self) -> (BigUint, BigUint, BigUint) {
        //} -> (Modulo, Modulo, Modulo) {
        let exponent = BigUint::from(self.exponent);
        dbg!(self.sig1.clone());
        dbg!(&exponent);
        dbg!(&self.n);
        let hash1 = self.sig1.modpow(&exponent, &self.n);
        let hash2 = self.sig2.modpow(&exponent, &self.n);
        let hash3 = self.sig3.modpow(&exponent, &self.n);

        // dbg!(&hash1);
        // dbg!(&hash2);
        // dbg!(&hash3);
        (hash1, hash2, hash3)
    }
}

impl Default for Signature {
    fn default() -> Self {
        Self::new()
    }
}
