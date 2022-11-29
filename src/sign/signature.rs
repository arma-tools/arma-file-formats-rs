use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;

use crate::core::deku_util::read_biguint;
use crate::core::deku_util::read_string_zt;
use crate::core::deku_util::write_biguint;
use crate::core::deku_util::write_string_zt;
use crate::core::write::WriteExtTrait;
use crate::errors::RvffError;
use deku::DekuContainerRead;
use deku::DekuEnumExt;
use deku::DekuError;
use deku::{DekuContainerWrite, DekuRead, DekuUpdate, DekuWrite};
use rsa::BigUint;

const EXTENSION: &str = "bisign";

#[derive(Eq, PartialEq, Debug, DekuRead, DekuWrite)]
pub struct Signature {
    #[deku(
        reader = "read_string_zt(deku::rest)",
        writer = "write_string_zt(deku::output, &self.authority)"
    )]
    pub authority: String,

    #[deku(assert_eq = "148")]
    unk1: u32,
    #[deku(assert_eq = "518")]
    unk2: u32,
    #[deku(assert_eq = "9216")]
    unk3: u32,
    #[deku(assert_eq = "826364754")]
    unk4: u32,

    #[deku(update = "self.n.to_bytes_le().len()*8")]
    pub n_length: u32,
    pub exponent: u32,

    #[deku(
        reader = "read_biguint(deku::rest, *n_length as usize /8)",
        writer = "write_biguint(deku::output, &self.n)"
    )]
    pub n: BigUint,

    #[deku(update = "self.sig1.to_bytes_le().len()")]
    pub sig1_length: u32,
    #[deku(
        reader = "read_biguint(deku::rest, *sig1_length as usize)",
        writer = "write_biguint(deku::output, &self.sig1)"
    )]
    pub sig1: BigUint,

    pub version: SignVersion,

    #[deku(update = "self.sig2.to_bytes_le().len()")]
    pub sig2_length: u32,
    #[deku(
        reader = "read_biguint(deku::rest, *sig2_length as usize)",
        writer = "write_biguint(deku::output, &self.sig2)"
    )]
    pub sig2: BigUint,

    #[deku(update = "self.sig3.to_bytes_le().len()")]
    pub sig3_length: u32,
    #[deku(
        reader = "read_biguint(deku::rest, *sig3_length as usize)",
        writer = "write_biguint(deku::output, &self.sig3)"
    )]
    pub sig3: BigUint,
}

#[derive(Eq, PartialEq, Debug, DekuRead, DekuWrite, Clone, Copy)]
#[deku(type = "u32")]
pub enum SignVersion {
    #[deku(id = "0x02")]
    V2 = 2,
    #[deku(id = "0x03")]
    V3 = 3,
}

impl Signature {
    pub fn new() -> Self {
        Signature {
            authority: String::default(),
            unk1: 148,
            unk2: 518,
            unk3: 9216,
            unk4: 826364754,
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

    pub fn from_stream<R>(reader: &mut R) -> Result<Signature, RvffError>
    where
        R: Read,
    {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;

        let (_, pub_key) = Signature::from_bytes((&buf, 0))?;

        Ok(pub_key)
    }

    pub fn write_file<P: AsRef<Path>>(&self, path: P) -> Result<(), RvffError> {
        let path: &Path = &path.as_ref().with_extension(EXTENSION);

        let mut file = File::create(path)?;
        file.write_bytes(&self.write_data()?)?;
        Ok(())
    }

    pub fn write_data(&self) -> Result<Vec<u8>, RvffError> {
        Ok(self.to_bytes()?)
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
