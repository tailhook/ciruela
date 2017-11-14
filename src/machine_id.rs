use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::str::FromStr;

use hex::{FromHex, ToHex};
use hexlify::Hex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Visitor, Error};


#[derive(Hash, PartialEq, Eq, Clone)]
pub struct MachineId([u8; 16]);


struct IdVisitor;


impl MachineId {
    pub fn read() -> Result<MachineId, io::Error> {
        let mut buf = String::with_capacity(33);
        File::open("/etc/machine-id")
        .and_then(|mut f| f.read_to_string(&mut buf))
        .and_then(|bytes| if bytes != 32 && bytes != 33  {
            return Err(io::Error::new(io::ErrorKind::Other,
                "Wrong length of /etc/machine-id"));
        } else {
            let vec: Vec<u8> = FromHex::from_hex(&buf[..32])
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            let mut ar = [0u8; 16];
            ar.copy_from_slice(&vec);
            Ok(MachineId(ar))
        })
    }
}

impl FromStr for MachineId {
    type Err = String;
    fn from_str(hash: &str) -> Result<MachineId, String> {
        if hash.len() != 32 {
            return Err(String::from(
                "MachineId must be exactly 32 hex chars"));
        }
        let vec: Vec<u8> = FromHex::from_hex(&hash[..])
            .map_err(|e| format!("MachineId invalid: {}", e))?;
        let mut val = [0u8; 16];
        val.copy_from_slice(&vec);
        Ok(MachineId(val))
    }
}

impl fmt::Debug for MachineId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MachineId({})", Hex(&self.0))
    }
}

impl fmt::Display for MachineId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.write_hex(f)
    }
}

impl<'a> Visitor<'a> for IdVisitor {
    type Value = MachineId;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bytes")
    }
    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
        where E: Error
    {
        if value.len() == 16 {
            let mut array = [0u8; 16];
            array.copy_from_slice(value);
            Ok(MachineId(array))
        } else {
            return Err(E::invalid_length(value.len(), &self));
        }
    }
}

impl<'a> Deserialize<'a> for MachineId {
    fn deserialize<D>(deserializer: D) -> Result<MachineId, D::Error>
        where D: Deserializer<'a>
    {
        deserializer.deserialize_bytes(IdVisitor)
    }
}

impl Serialize for MachineId {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        ser.serialize_bytes(&self.0)
    }
}
