use std::fmt;

use base64;
use serde::{Serialize, Serializer};
use serde::{Deserialize, Deserializer};
use serde_cbor::ser::Serializer as Cbor;


pub enum Signature {
    SshEd25519([u8; 64]),
}

pub struct SigData<'a> {
    pub path: &'a str,
    pub image: &'a [u8],
    pub timestamp: u64,
}

pub struct Bytes<'a>(&'a [u8]);

pub fn sign_default(src: SigData) -> Vec<Signature> {
    let mut buf = Vec::with_capacity(100);
    (src.path, Bytes(src.image), src.timestamp)
        .serialize(&mut Cbor::new(&mut buf))
        .expect("Can always serialize signature data");
    println!("Data {:?}", String::from_utf8_lossy(&buf));
    unimplemented!();
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        use self::Signature::*;

        match *self {
            SshEd25519(val) => (
                "ssh-ed25519", Bytes(&val[..])
                ).serialize(serializer)
        }
    }
}

impl<'a> Serialize for Bytes<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        use self::Signature::*;
        serializer.serialize_bytes(&self.0[..])
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

impl Clone for Signature {
    fn clone(&self) -> Signature {
        use self::Signature::*;

        match *self {
            SshEd25519(val) => SshEd25519(val),
        }
    }
}