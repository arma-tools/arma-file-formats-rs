use crate::{
    core::{
        binrw_utils::{read_biguint, write_biguint},
        write::WriteExtTrait,
    },
    errors::RvffError,
};
use binrw::{binrw, BinRead, Endian};
use binrw::{BinWrite, NullString};
use std::{
    fs::File,
    io::{Cursor, Seek},
};
use std::{
    io::{BufReader, Read},
    path::Path,
};

use rsa::BigUint;
use rsa::{PublicKeyParts, RsaPrivateKey};

pub(crate) const KEY_LENGTH: u32 = 1024;
const EXPONENT: u32 = 65537;

const EXTENSION: &str = "biprivatekey";

#[derive(Eq, PartialEq, Clone, Debug)]
#[binrw]
#[brw(little)]
pub struct PrivateKey {
    pub authority: NullString,

    #[br(assert(unk1 == 596))]
    #[bw(assert(unk1 == &596))]
    unk1: u32,
    #[br(assert(unk2 == 519))]
    #[bw(assert(unk2 == &519))]
    unk2: u32,
    #[br(assert(unk3 == 9216))]
    #[bw(assert(unk3 == &9216))]
    unk3: u32,
    #[br(assert(unk4 == 843141970))]
    #[bw(assert(unk4 == &843141970))]
    unk4: u32,

    //#[deku(update = "self.n.to_bytes_le().len()*8")]
    //#[bw(calc = (n.to_bytes_le().len() * 8) as u32)]
    n_length: u32,
    pub exponent: u32,

    // #[deku(
    //     reader = "read_biguint(deku::rest, *n_length as usize /8)",
    //     writer = "write_biguint(deku::output, &self.n)"
    // )]
    #[br(args((n_length as usize / 8)))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub n: BigUint,

    // #[deku(
    //     reader = "read_biguint(deku::rest, *n_length as usize /16)",
    //     writer = "write_biguint(deku::output, &self.p)"
    // )]
    #[br(args((n_length as usize / 16)))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub p: BigUint,

    // #[deku(
    //     reader = "read_biguint(deku::rest, *n_length as usize /16)",
    //     writer = "write_biguint(deku::output, &self.q)"
    // )]
    #[br(args((n_length as usize / 16)))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub q: BigUint,

    // #[deku(
    //     reader = "read_biguint(deku::rest, *n_length as usize /16)",
    //     writer = "write_biguint(deku::output, &self.dmp1)"
    // )]
    #[br(args((n_length as usize / 16)))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub dmp1: BigUint,

    // #[deku(
    //     reader = "read_biguint(deku::rest, *n_length as usize /16)",
    //     writer = "write_biguint(deku::output, &self.dmq1)"
    // )]
    #[br(args((n_length as usize / 16)))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub dmq1: BigUint,

    // #[deku(
    //     reader = "read_biguint(deku::rest, *n_length as usize /16)",
    //     writer = "write_biguint(deku::output, &self.iqmp)"
    // )]
    #[br(args((n_length as usize / 16)))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub iqmp: BigUint,

    // #[deku(
    //     reader = "read_biguint(deku::rest, *n_length as usize /8)",
    //     writer = "write_biguint(deku::output, &self.d)"
    // )]
    #[br(args((n_length as usize / 8)))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub d: BigUint,
}

impl PrivateKey {
    fn new() -> PrivateKey {
        Self {
            authority: String::default().into(),
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
        R: Read + Seek,
    {
        let prv_key = PrivateKey::read_options(reader, Endian::Little, ())?;
        Ok(prv_key)
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

        PrivateKey::write(self, &mut cursor)?;

        Ok(buf)
    }

    pub fn generate<S: Into<String>>(authority: S) -> Self {
        let mut priv_key = PrivateKey::new();

        let mut rng = rand::thread_rng();
        let rsa_priv_key =
            RsaPrivateKey::new_with_exp(&mut rng, KEY_LENGTH as usize, &BigUint::from(EXPONENT))
                .unwrap();

        priv_key.authority = Into::<String>::into(authority).into();
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
