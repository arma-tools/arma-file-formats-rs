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
use rsa::PublicKeyParts;
use rsa::RsaPrivateKey;

pub(crate) const KEY_LENGTH: u32 = 1024;
const EXPONENT: u32 = 65537;

const EXTENSION: &str = "biprivatekey";

#[derive(Eq, PartialEq, Clone, Debug, DekuRead, DekuWrite)]
pub struct PrivateKey {
    #[deku(
        reader = "read_string_zt(deku::rest)",
        writer = "write_string_zt(deku::output, &self.authority)"
    )]
    pub authority: String,

    #[deku(assert_eq = "596")]
    unk1: u32,
    #[deku(assert_eq = "519")]
    unk2: u32,
    #[deku(assert_eq = "9216")]
    unk3: u32,
    #[deku(assert_eq = "843141970")]
    unk4: u32,

    #[deku(update = "self.n.to_bytes_le().len()*8")]
    pub n_length: u32,
    pub exponent: u32,

    #[deku(
        reader = "read_biguint(deku::rest, *n_length as usize /8)",
        writer = "write_biguint(deku::output, &self.n)"
    )]
    pub n: BigUint,

    #[deku(
        reader = "read_biguint(deku::rest, *n_length as usize /16)",
        writer = "write_biguint(deku::output, &self.p)"
    )]
    pub p: BigUint,

    #[deku(
        reader = "read_biguint(deku::rest, *n_length as usize /16)",
        writer = "write_biguint(deku::output, &self.q)"
    )]
    pub q: BigUint,

    #[deku(
        reader = "read_biguint(deku::rest, *n_length as usize /16)",
        writer = "write_biguint(deku::output, &self.dmp1)"
    )]
    pub dmp1: BigUint,

    #[deku(
        reader = "read_biguint(deku::rest, *n_length as usize /16)",
        writer = "write_biguint(deku::output, &self.dmq1)"
    )]
    pub dmq1: BigUint,

    #[deku(
        reader = "read_biguint(deku::rest, *n_length as usize /16)",
        writer = "write_biguint(deku::output, &self.iqmp)"
    )]
    pub iqmp: BigUint,

    #[deku(
        reader = "read_biguint(deku::rest, *n_length as usize /8)",
        writer = "write_biguint(deku::output, &self.d)"
    )]
    pub d: BigUint,
}

impl PrivateKey {
    fn new() -> PrivateKey {
        Self {
            authority: String::default(),
            unk1: 596,
            unk2: 519,
            unk3: 9216,
            unk4: 843141970,
            n_length: 0,
            exponent: 0,
            n: BigUint::default(),
            p: BigUint::default(),
            q: BigUint::default(),
            dmp1: BigUint::default(),
            dmq1: BigUint::default(),
            iqmp: BigUint::default(),
            d: BigUint::default(),
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, RvffError> {
        let file = File::open(path)?;
        let mut buf_reader = BufReader::new(file);
        Self::from_stream(&mut buf_reader)
    }

    pub fn from_stream<R>(reader: &mut R) -> Result<PrivateKey, RvffError>
    where
        R: Read,
    {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;

        let (_, pub_key) = PrivateKey::from_bytes((&buf, 0))?;

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

    pub fn generate<S: Into<String>>(authority: S) -> Self {
        let mut priv_key = PrivateKey::new();

        let mut rng = rand::thread_rng();
        let rsa_priv_key =
            RsaPrivateKey::new_with_exp(&mut rng, KEY_LENGTH as usize, &BigUint::from(EXPONENT))
                .unwrap();

        priv_key.authority = authority.into();
        priv_key.n_length = KEY_LENGTH;
        priv_key.exponent = EXPONENT;
        priv_key.n = rsa_priv_key.n().to_owned();
        priv_key.p = rsa_priv_key.primes()[0].clone();
        priv_key.q = rsa_priv_key.primes()[1].clone();
        priv_key.dmp1 = rsa_priv_key.dp().unwrap_or(&BigUint::default()).to_owned();
        priv_key.dmq1 = rsa_priv_key.dq().unwrap_or(&BigUint::default()).to_owned();
        priv_key.iqmp = rsa_priv_key
            .qinv()
            .unwrap() //_or(&BigInt::default())
            .to_owned()
            .to_biguint()
            .unwrap();
        priv_key.d = rsa_priv_key.d().to_owned();

        priv_key
    }
}

impl Default for PrivateKey {
    fn default() -> Self {
        Self::new()
    }
}
