use std::time::SystemTime;

use serde::{Serialize, Deserialize, Serializer, Deserializer};

use {ImageId};
use proto::Signature;
use time::{to_ms, from_ms};


#[derive(Debug)]
pub struct SignatureEntry {
    pub timestamp: SystemTime,
    pub signature: Signature,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct State {
    pub image: ImageId,
    pub signatures: Vec<SignatureEntry>,
}

impl Serialize for SignatureEntry {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        (to_ms(self.timestamp), &self.signature).serialize(s)
    }
}

impl Deserialize for SignatureEntry {
    fn deserialize<D: Deserializer>(deserializer: D) -> Result<Self, D::Error>
    {
        let (ts, sig) = Deserialize::deserialize(deserializer)?;
        Ok(SignatureEntry {
            timestamp: from_ms(ts),
            signature: sig,
        })
    }
}
