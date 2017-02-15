use std::fmt;

use base64;
use serde::{Serialize, Serializer};
use serde::{Deserialize, Deserializer};


pub enum Signature {
    SshEd25519([u8; 64]),
}

pub struct SigData<'a> {
    pub path: &'a str,
    pub image: &'a [u8],
    pub timestamp: u64,
}

pub fn sign_default(src: SigData) -> Signature {
    unimplemented!();
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        use self::Signature::*;

        match *self {
            SshEd25519(val) => ("ssh-ed25519", &val[..]).serialize(serializer)
        }
    }
}

impl Deserialize for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        use self::Signature::*;
        unimplemented!();
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Signature::*;

        match *self {
            SshEd25519(val) => {
                write!(f, "SshEd25519({})", base64::encode(&val[..]))
            }
        }
    }
}
