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
use deku::{DekuContainerWrite, DekuRead, DekuUpdate, DekuWrite};
use rsa::BigUint;

use super::PrivateKey;

const EXTENSION: &str = "bikey";

#[derive(Eq, PartialEq, Debug, DekuRead, DekuWrite)]
pub struct PublicKey {
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
    //#[deku(count = "n_length/8")]
    pub n: BigUint,
}

impl PublicKey {
    pub fn new() -> Self {
        PublicKey {
            authority: String::new(),
            unk1: 148,
            unk2: 518,
            unk3: 9216,
            unk4: 826364754,
            n_length: 0,
            exponent: 0,
            n: BigUint::default(),
        }
    }
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, RvffError> {
        let file = File::open(path)?;
        let mut buf_reader = BufReader::new(file);
        Self::from_stream(&mut buf_reader)
    }

    pub fn from_stream<R>(reader: &mut R) -> Result<PublicKey, RvffError>
    where
        R: Read,
    {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;

        let (_, pub_key) = PublicKey::from_bytes((&buf, 0))?;

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
}

impl Default for PublicKey {
    fn default() -> Self {
        Self::new()
    }
}

impl From<PrivateKey> for PublicKey {
    fn from(priv_key: PrivateKey) -> Self {
        let mut pub_key = PublicKey::new();
        pub_key.authority = priv_key.authority;
        pub_key.exponent = priv_key.exponent;
        pub_key.n_length = priv_key.n_length;
        pub_key.n = priv_key.n;
        pub_key
    }
}
