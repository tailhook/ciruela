use std::fmt;

use base64;
use crypto::ed25519;
use hex::ToHex;
use serde::{Serialize, Serializer};
use serde::{Deserialize, Deserializer};
use serde::de::{Visitor, SeqVisitor, Error};
use serde_cbor::ser::Serializer as Cbor;
use ssh_keys::PrivateKey;


pub enum Signature {
    SshEd25519([u8; 64]),
}

enum SignatureType {
    SshEd25519,
}

struct SignatureVisitor;
struct TypeVisitor;
struct Ed25519Visitor;
struct Ed25519([u8; 64]);

const TYPES: &'static [&'static str] = &[
    "ssh-ed25519",
    ];

pub struct SigData<'a> {
    pub path: &'a str,
    pub image: &'a [u8],
    pub timestamp: u64,
}

pub struct Bytes<'a>(&'a [u8]);

pub fn sign(src: SigData, keys: &[PrivateKey]) -> Vec<Signature> {
    let mut buf = Vec::with_capacity(100);
    (src.path, Bytes(src.image), src.timestamp)
        .serialize(&mut Cbor::new(&mut buf))
        .expect("Can always serialize signature data");
    let mut res = Vec::new();
    for key in keys {
        let signature = match *key {
            PrivateKey::Ed25519(ref bytes) => {
                Signature::SshEd25519(ed25519::signature(&buf[..], &bytes[..]))
            }
            _ => {
                unimplemented!();
            }
        };
        res.push(signature);
    }
    info!("Image {}[{}] signed with {} keys",
        src.path, src.image.to_hex(), res.len());
    return res;
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
        serializer.serialize_bytes(&self.0[..])
    }
}

impl Deserialize for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_seq(SignatureVisitor)
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

impl Visitor for TypeVisitor {
    type Value = SignatureType;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&TYPES.join(", "))
    }

    fn visit_str<E>(self, value: &str) -> Result<SignatureType, E>
        where E: Error
    {
        match value {
            "ssh-ed25519" => Ok(SignatureType::SshEd25519),
            _ => Err(Error::unknown_field(value, TYPES)),
        }
    }
}

impl Visitor for Ed25519Visitor {
    type Value = Ed25519;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("64 bytes")
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Ed25519, E>
        where E: Error
    {
        if value.len() != 64 {
            return Err(Error::invalid_length(1, &self));
        }
        let mut res = [0u8; 64];
        res.copy_from_slice(value);
        return Ok(Ed25519(res));
    }
}

impl Deserialize for SignatureType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_str(TypeVisitor)
    }
}

impl Deserialize for Ed25519 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_bytes(Ed25519Visitor)
    }
}

impl Visitor for SignatureVisitor {
    type Value = Signature;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("signature")
    }

    #[inline]
    fn visit_seq<V>(self, mut visitor: V) -> Result<Signature, V::Error>
        where V: SeqVisitor,
    {
        match visitor.visit()? {
            Some(SignatureType::SshEd25519) => match visitor.visit()? {
                Some(Ed25519(value)) => Ok(Signature::SshEd25519(value)),
                None => Err(Error::invalid_length(1, &self)),
            },
            None => Err(Error::custom("invalid signature type")),
        }
    }
}
