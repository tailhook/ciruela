/// Block hash
///
/// Basically it's an array of bytes which is hexlified when printing and
/// serialized as bytes. Currently only one size of hash is supported but more
/// lengths can be supported later.
use std::fmt;

use hexlify::Hex;

#[derive(Serialize)]
#[derive(Hash, PartialEq, Eq, Clone)]
pub struct BlockHash([u8; 32]);

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

