use std::fmt;
use std::io;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Visitor, Unexpected, Error};
use serde_cbor::ser::to_writer;

use hex::{FromHex};
use hexlify::Hex;
use blake2::digest::VariableOutput;
use digest_writer;
use blake2::{Blake2b, Digest};


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
    pub fn builder() -> digest_writer::Writer<Blake2b> {
        VariableOutput::new(32).expect("length is okay")
    }
    pub fn for_object<S: Serialize>(obj: &S) -> Hash {
        let mut dig = Hash::builder();
        dig.object(obj);
        return dig.done();
    }
    pub fn for_bytes(bytes: &[u8]) -> Hash {
        let mut dig = Hash::builder();
        dig.input(bytes);
        return dig.done();
    }
}

pub trait Builder: io::Write {
    fn done(self) -> Hash;
    fn object<S: Serialize>(&mut self, obj: &S);
}

impl Builder for digest_writer::Writer<Blake2b> {
    fn object<S: Serialize>(&mut self, obj: &S) {
        to_writer(self, obj).expect("can always serialize/hash structure");
    }
    fn done(self) -> Hash {
        let mut result = [0u8; 32];
        self.variable_result(&mut result).expect("length is okay");
        Hash(result)
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Hash({})", &Hex(&self.0))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &Hex(&self.0))
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
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where E: Error
    {
        if value.len() == 64 {
            let vec: Vec<u8> = FromHex::from_hex(value)
                .map_err(|_| E::invalid_value(
                    Unexpected::Str(value), &"hex coded string"))?;
            let mut val = [0u8; 32];
            val.copy_from_slice(&vec);
            Ok(Hash(val))
        } else {
            return Err(E::invalid_length(value.len(), &self));
        }
    }
}

impl<'a> Deserialize<'a> for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Hash, D::Error>
        where D: Deserializer<'a>
    {
        deserializer.deserialize_any(HashVisitor)
    }
}

impl Serialize for Hash {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        if ser.is_human_readable() {
            ser.serialize_str(&self.to_string())
        } else {
            ser.serialize_bytes(&self.0)
        }
    }
}
