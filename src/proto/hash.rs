use std::fmt;
use std::io;

use hex::ToHex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Visitor, Error};
use serde_cbor::ser::to_writer;

use digest_writer;
use blake2::{Blake2b, Digest};
use typenum::U32;


#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct Hash([u8; 32]);

struct HashVisitor;


impl Hash {
    pub fn new(hash: &[u8]) -> Hash {
        assert_eq!(hash.len(), 32);
        let mut val = [0u8; 32];
        val.copy_from_slice(hash);
        return Hash(val);
    }
    pub fn builder() -> digest_writer::Writer<Blake2b<U32>> {
        digest_writer::Writer::new(Blake2b::<U32>::new())
    }
    pub fn for_object<S: Serialize>(obj: &S) -> Hash {
        let mut dig = Hash::builder();
        dig.object(obj);
        return dig.done();
    }
}

pub trait Builder: io::Write {
    fn done(self) -> Hash;
    fn object<S: Serialize>(&mut self, obj: &S);
}

impl Builder for digest_writer::Writer<Blake2b<U32>> {
    fn object<S: Serialize>(&mut self, obj: &S) {
        to_writer(self, obj).expect("can always serialize/hash structure");
    }
    fn done(self) -> Hash {
        Hash::new(&self.into_inner().result()[..])
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Hash({})", self.0.to_hex())
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.to_hex())
    }
}

impl<'a> Visitor<'a> for HashVisitor {
    type Value = Hash;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bytes")
    }
    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
        where E: Error
    {
        if value.len() == 32 {
            let mut array = [0u8; 32];
            array.copy_from_slice(value);
            Ok(Hash(array))
        } else {
            return Err(E::invalid_length(value.len(), &self));
        }
    }
}

impl<'a> Deserialize<'a> for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Hash, D::Error>
        where D: Deserializer<'a>
    {
        deserializer.deserialize_bytes(HashVisitor)
    }
}

impl Serialize for Hash {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        ser.serialize_bytes(&self.0)
    }
}
