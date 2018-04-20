use std::fmt;
use std::sync::Arc;
use std::str::FromStr;

use hex::{ToHex, FromHex};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Visitor, Error};


#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct Hash([u8; 32]);

/// Image identifier
///
/// This is basically an array of bytes that is hexlified when printing.
///
/// The meaning of the type is the hash of all the entries in the index. So
/// basically it can be used as an identified in content-addressed store.
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct ImageId(Internal);

/// Error parsing hex string into image id
#[derive(Fail, Debug)]
#[fail(display="errors parding hexadecimal image id")]
pub struct ParseError;

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

impl FromStr for ImageId {
    type Err = ParseError;
    fn from_str(value: &str) -> Result<ImageId, ParseError> {
        if value.len() == 64 {
            let array:[u8; 32] = FromHex::from_hex(value)
                .map_err(|_| ParseError)?;
            Ok(ImageId(Internal::B32(array)))
        } else {
            Ok(ImageId(Internal::Other(Arc::new(
                Vec::from_hex(value).map_err(|_| ParseError)?
                .into_boxed_slice()))))
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
        f.write_str("ImageId(")?;
        match self.0 {
            Internal::B32(ref ar) => ar.write_hex(f)?,
            Internal::Other(ref slc) => slc.write_hex(f)?,
        }
        f.write_str(")")
    }
}

impl fmt::Display for ImageId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            // TODO(tailhook) optimize to be zero-allocation
            Internal::B32(ref ar) => ar.write_hex(f),
            Internal::Other(ref slc) => slc.write_hex(f),
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
        if ser.is_human_readable() {
            ser.serialize_str(&self.to_string())
        } else {
            match self.0 {
                Internal::B32(ref ar) => ser.serialize_bytes(ar),
                Internal::Other(ref slc) => ser.serialize_bytes(slc),
            }
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
