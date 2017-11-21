use std::time::SystemTime;

use serde::{Serialize, Deserialize, Serializer, Deserializer};

use {ImageId};
use proto::Signature;
use time_util::{to_ms, from_ms};


#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
// Note everything here, must be stable-serialized
pub struct SignatureEntry {
    pub timestamp: SystemTime,
    pub signature: Signature,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
// Note everything here, must be stable-serialized
pub struct State {
    pub image: ImageId,
    pub signatures: Vec<SignatureEntry>,
}

impl Serialize for SignatureEntry {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        (to_ms(self.timestamp), &self.signature).serialize(s)
    }
}

impl<'a> Deserialize<'a> for SignatureEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'a>,
    {
        let (ts, sig) = Deserialize::deserialize(deserializer)?;
        Ok(SignatureEntry {
            timestamp: from_ms(ts),
            signature: sig,
        })
    }
}
