/// Block hash
///
/// Basically it's an array of bytes which is hexlified when printing and
/// serialized as bytes. Currently only one size of hash is supported but more
/// lengths can be supported later.
use std::fmt;

use hexlify::Hex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Visitor, Error};
use blake2::{Blake2b, Digest};
use typenum::U32;

/// Hash of a block
///
/// This is an array of bytes. Currently only 32bytes hashes are supported
/// but more may be added later.
#[derive(Hash, PartialEq, Eq, Clone)]
pub struct BlockHash([u8; 32]);

struct HashVisitor;

impl BlockHash {

    /// Create a BlockHash object from a byte slice
    ///
    /// Returns None if this byte size is unsupported
    pub fn from_bytes(bytes: &[u8]) -> Option<BlockHash> {
        if bytes.len() == 32 {
            let mut ar = [0u8; 32];
            ar.copy_from_slice(bytes);
            return Some(BlockHash(ar));
        }
        return None;
    }
    pub(crate) fn for_bytes(bytes: &[u8]) -> BlockHash {
        let mut hash = Blake2b::<U32>::new();
        hash.input(bytes);
        return BlockHash::from_bytes(&hash.result()[..]).unwrap();
    }
}

impl fmt::Debug for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BlockHash({})", &Hex(&self.0))
    }
}

impl fmt::Display for BlockHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &Hex(&self.0))
    }
}

impl<'a> Visitor<'a> for HashVisitor {
    type Value = BlockHash;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bytes")
    }
    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
        where E: Error
    {
        if value.len() == 32 {
            let mut array = [0u8; 32];
            array.copy_from_slice(value);
            Ok(BlockHash(array))
        } else {
            return Err(E::invalid_length(value.len(), &self));
        }
    }
}

impl<'a> Deserialize<'a> for BlockHash {
    fn deserialize<D>(deserializer: D) -> Result<BlockHash, D::Error>
        where D: Deserializer<'a>
    {
        deserializer.deserialize_bytes(HashVisitor)
    }
}

impl Serialize for BlockHash {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        ser.serialize_bytes(&self.0)
    }
}
