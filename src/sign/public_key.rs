use crate::{
    core::{
        binrw_utils::{read_biguint, write_biguint},
        write::WriteExtTrait,
    },
    errors::RvffError,
};
use binrw::{binrw, BinRead, Endian};
use binrw::{BinWrite, NullString};
use rsa::BigUint;
use std::io::Read;
use std::io::{BufReader, Seek};
use std::path::Path;
use std::{fs::File, io::Cursor};

use super::PrivateKey;

const EXTENSION: &str = "bikey";

#[derive(Eq, PartialEq, Debug)]
#[binrw]
#[brw(little)]
pub struct PublicKey {
    // #[deku(
    //     reader = "read_string_zt(deku::rest)",
    //     writer = "write_string_zt(deku::output, &self.authority)"
    // )]
    pub authority: NullString,

    #[br(assert(unk1 == 148))]
    #[bw(assert(unk1 == &148))]
    // #[deku(assert_eq = "148")]
    unk1: u32,
    // #[deku(assert_eq = "518")]
    #[br(assert(unk2 == 518))]
    #[bw(assert(unk2 == &518))]
    unk2: u32,
    // #[deku(assert_eq = "9216")]
    #[br(assert(unk3 == 9216))]
    #[bw(assert(unk3 == &9216))]
    unk3: u32,
    // #[deku(assert_eq = "826364754")]
    #[br(assert(unk4 == 826364754))]
    #[bw(assert(unk4 == &826364754))]
    unk4: u32,

    // #[deku(update = "self.n.to_bytes_le().len()*8")]
    n_length: u32,
    pub exponent: u32,

    // #[deku(
    //     reader = "read_biguint(deku::rest, *n_length as usize /8)",
    //     writer = "write_biguint(deku::output, &self.n)"
    // )]
    //#[deku(count = "n_length/8")]
    #[br(args((n_length as usize / 8)))]
    #[br(parse_with = read_biguint)]
    #[bw(write_with = write_biguint)]
    pub n: BigUint,
}

impl PublicKey {
    pub fn new() -> Self {
        PublicKey {
            authority: String::default().into(),
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
        R: Read + Seek,
    {
        let pub_key = PublicKey::read_options(reader, Endian::Little, ())?;
        Ok(pub_key)
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

        PublicKey::write(self, &mut cursor)?;

        Ok(buf)
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
        pub_key.n_length = (priv_key.n.to_bytes_le().len() * 8) as u32;
        pub_key.n = priv_key.n;
        pub_key
    }
}
