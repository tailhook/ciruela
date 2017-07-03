use std::fmt;
use std::sync::Arc;

use hex::ToHex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Visitor, Error};

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct ImageId(Internal);

struct ImageIdVisitor;

#[derive(Clone, Hash, PartialEq, Eq)]
enum Internal {
    B32([u8; 32]),
    Other(Arc<Box<[u8]>>),
}

impl<'a> From<&'a [u8]> for ImageId {
    fn from(value: &[u8]) -> ImageId {
        let value = value.as_ref();
        if value.len() == 32 {
            let mut array = [0u8; 32];
            array.copy_from_slice(value);
            ImageId(Internal::B32(array))
        } else {
            ImageId(Internal::Other(Arc::new(
                value.to_vec().into_boxed_slice())))
        }
    }
}

impl From<Vec<u8>> for ImageId {
    fn from(value: Vec<u8>) -> ImageId {
        if value.len() == 32 {
            let mut array = [0u8; 32];
            array.copy_from_slice(&value);
            ImageId(Internal::B32(array))
        } else {
            ImageId(Internal::Other(Arc::new(value.into_boxed_slice())))
        }
    }
}

impl fmt::Debug for ImageId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Internal::B32(ref ar) => write!(f, "ImageId({})", ar.to_hex()),
            Internal::Other(ref slc) => write!(f, "ImageId({})", slc.to_hex()),
        }
    }
}

impl fmt::Display for ImageId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            // TODO(tailhook) optimize to be zero-allocation
            Internal::B32(ref ar) => write!(f, "{}", ar.to_hex()),
            Internal::Other(ref slc) => write!(f, "{}", slc.to_hex()),
        }
    }
}

impl<'a> Visitor<'a> for ImageIdVisitor {
    type Value = ImageId;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bytes")
    }
    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
        where E: Error
    {
        if value.len() == 32 {
            let mut array = [0u8; 32];
            array.copy_from_slice(value);
            Ok(ImageId(Internal::B32(array)))
        } else {
            Ok(ImageId(Internal::Other(Arc::new(
                value.to_vec().into_boxed_slice()))))
        }
    }
}

impl<'a> Deserialize<'a> for ImageId {
    fn deserialize<D>(deserializer: D) -> Result<ImageId, D::Error>
        where D: Deserializer<'a>
    {
        deserializer.deserialize_bytes(ImageIdVisitor)
    }
}

impl Serialize for ImageId {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        match self.0 {
            Internal::B32(ref ar) => ser.serialize_bytes(ar),
            Internal::Other(ref slc) => ser.serialize_bytes(slc),
        }
    }
}

impl AsRef<[u8]> for ImageId {
    fn as_ref(&self) -> &[u8] {
        match self.0 {
            Internal::B32(ref ar) => &ar[..],
            Internal::Other(ref slc) => &slc[..],
        }
    }
}
