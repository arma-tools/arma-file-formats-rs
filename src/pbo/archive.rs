use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

use indexmap::IndexMap;
use rsa::BigUint;
use sha1::digest::Output;
use sha1::{Digest, Sha1};

use crate::core::read::ReadExtTrait;
use crate::errors::RvffError;
use crate::sign::{PrivateKey, PublicKey, SignVersion, Signature, KEY_LENGTH};

use super::entry::Entry;

const PBO_MAGIC: &str = "sreV";

const V2_EXCLUDE_LIST: [&str; 13] = [
    "paa", "jpg", "p3d", "tga", "rvmat", "lip", "ogg", "wss", "png", "rtm", "pac", "fxy", "wrp",
];

const V3_INCLUDE_LIST: [&str; 11] = [
    "sqf", "inc", "bikb", "ext", "fsm", "sqm", "hpp", "cfg", "sqs", "h", "sqfc",
];

pub struct Pbo {
    pub properties: IndexMap<String, String>,

    pub entries: IndexMap<String, Entry>,
    pub hash: Vec<u8>,
}

impl Pbo {
    pub fn new() -> Self {
        Pbo {
            properties: IndexMap::new(),
            entries: IndexMap::new(),
            hash: Vec::new(),
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, RvffError> {
        let file = File::open(path)?;
        let mut buf_reader = BufReader::new(file);
        Pbo::from_stream(&mut buf_reader)
    }

    pub fn from_stream<R>(reader: &mut R) -> Result<Self, RvffError>
    where
        R: BufRead + Seek,
    {
        let mut pbo = Pbo::new();
        pbo.read(reader, false)?;
        Ok(pbo)
    }

    pub(crate) fn read<R>(&mut self, reader: &mut R, skip_data: bool) -> Result<(), RvffError>
    where
        R: BufRead + Seek,
    {
        if reader.read_u8()? != 0
            || reader.read_string(4)? != PBO_MAGIC
            || reader.read_bytes(16)?.into_iter().all(|x| x != 0)
        {
            return Err(RvffError::InvalidFileError);
        }

        while reader.peek_u8()? != 0 {
            self.properties
                .insert(reader.read_string_zt()?, reader.read_string_zt()?);
        }
        reader.read_u8()?;

        while reader.peek_u8()? != 0 {
            let mut entry = Entry::new();
            entry.read(reader)?;

            self.entries.insert(entry.filename.clone(), entry);
        }

        reader.read_bytes(21)?;
        let mut data_pos = reader.stream_position()?;

        for entry in &mut self.entries {
            entry.1.data_offset = data_pos;
            data_pos += entry.1.data_size as u64;
        }

        if !skip_data {
            for entry in &mut self.entries {
                entry.1.read_data(reader)?;
            }
        }

        reader.seek(SeekFrom::Start(data_pos))?;
        if reader.read_u8()? != 0 {
            return Err(RvffError::InvalidFileError);
        }

        self.hash = reader.read_bytes(20)?;

        Ok(())
    }

    pub(crate) fn get_entry<R>(
        &mut self,
        entry_path: String,
        reader: &mut R,
    ) -> Result<Option<Entry>, RvffError>
    where
        R: BufRead + Seek,
    {
        if let Some(entry) = self.entries.get_mut(&entry_path) {
            if entry.data.is_empty() {
                entry.read_data(reader)?;
            }
            Ok(Some(entry.clone()))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn generate_hashes(
        &self,
        version: SignVersion,
        length: u32,
    ) -> (BigUint, BigUint, BigUint) {
        let checksum = &self.hash;

        let hash1 = checksum.as_slice();

        // println!("Namehash");
        // dbg!(&self.namehash());

        let mut hash2 = Sha1::new();
        hash2.update(hash1);
        hash2.update(self.namehash());

        if let Some(prefix) = self.properties.get("prefix") {
            hash2.update(prefix.as_bytes());
            if !prefix.ends_with('\\') {
                hash2.update(b"\\");
            }
        }

        // dbg!(&hash1);
        //dbg!(&hash2.finalize());

        let mut hash3 = Sha1::new();
        hash3.update(self.filehash(version));
        hash3.update(self.namehash());
        if let Some(prefix) = self.properties.get("prefix") {
            hash3.update(prefix.as_bytes());
            if !prefix.ends_with('\\') {
                hash3.update(b"\\");
            }
        }
        (
            self.pad_hash(hash1, (length / 8) as usize),
            self.pad_hash(&hash2.finalize(), (length / 8) as usize),
            self.pad_hash(&hash3.finalize(), (length / 8) as usize),
        )
    }

    pub fn sign(&self, version: SignVersion, priv_key: &PrivateKey) -> Signature {
        let (hash1, hash2, hash3) = self.generate_hashes(version, KEY_LENGTH);

        let mut sig = Signature::new();
        sig.version = version;
        sig.authority = priv_key.authority.clone();
        sig.exponent = priv_key.exponent;
        sig.n_length = (priv_key.n.to_bytes_le().len() * 8) as u32;
        sig.n = priv_key.n.clone();

        sig.sig1 = hash1.modpow(&priv_key.d, &priv_key.n);
        sig.sig1_length = sig.sig1.to_bytes_le().len() as u32;

        sig.sig2 = hash2.modpow(&priv_key.d, &priv_key.n);
        sig.sig2_length = sig.sig2.to_bytes_le().len() as u32;

        sig.sig3 = hash3.modpow(&priv_key.d, &priv_key.n);
        sig.sig3_length = sig.sig3.to_bytes_le().len() as u32;

        sig
    }

    pub fn verify(&self, public_key: &PublicKey, signature: &Signature) -> anyhow::Result<()> {
        if public_key.authority != signature.authority {
            panic!("auth not same");
        }

        // Pbo sorted?
        let (pbo_hash1, pbo_hash2, pbo_hash3) =
            self.generate_hashes(signature.version, signature.n_length);

        let (sign_hash1, sign_hash2, sign_hash3) = signature.get_hashes();

        dbg!(&sign_hash1);
        dbg!(&pbo_hash1);
        dbg!(&sign_hash2);
        dbg!(&pbo_hash2);
        dbg!(&sign_hash3);
        dbg!(&pbo_hash3);

        if sign_hash1 != pbo_hash1 {
            panic!("hash1 not same");
        }

        if sign_hash2 != pbo_hash2 {
            panic!("hash2 not same");
        }

        if sign_hash3 != pbo_hash3 {
            panic!("hash3 not same");
        }

        Ok(())
    }

    pub(crate) fn pad_hash(&self, hash: &[u8], size: usize) -> BigUint {
        let mut data: Vec<u8> = vec![0, 1];
        data.resize(size - 36, 255);
        data.extend(b"\x00\x30\x21\x30\x09\x06\x05\x2b");
        data.extend(b"\x0e\x03\x02\x1a\x05\x00\x04\x14");
        data.extend(hash);
        BigUint::from_bytes_be(&data)
        //UInt::from_le_slice(&data)
    }

    pub(crate) fn namehash(&self) -> Output<Sha1> {
        let mut hash = Sha1::new();

        for entry in self.entries.iter() {
            if entry.1.data.is_empty() {
                continue;
            }

            hash.update(entry.0.replace('/', "\\").to_lowercase().as_bytes());
        }

        hash.finalize()
    }

    pub(crate) fn filehash(&self, version: SignVersion) -> Output<Sha1> {
        let mut hash = Sha1::new();

        let mut empty = true;

        for entry in self.entries.iter() {
            let file_ext = entry.0.split('.').last().unwrap();

            if V2_EXCLUDE_LIST.contains(&file_ext) {
                continue;
            }

            if !V3_INCLUDE_LIST.contains(&file_ext) {
                continue;
            }

            hash.update(&entry.1.data);
            empty = false;
        }

        if empty {
            match version {
                SignVersion::V2 => hash.update(b"nothing"),
                SignVersion::V3 => hash.update(b"gnihton"),
            }
        }

        hash.finalize()
    }
}

impl Default for Pbo {
    fn default() -> Self {
        Self::new()
    }
}
