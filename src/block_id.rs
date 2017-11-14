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

