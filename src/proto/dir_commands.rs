use std::path::PathBuf;
use std::time::SystemTime;

use proto::{Signature, Request};


#[derive(Serialize, Deserialize, Debug)]
pub struct AppendDir {
    pub path: PathBuf,
    pub image: Vec<u8>,
    #[serde(deserialize_with="::proto::serializers::read_timestamp")]
    #[serde(serialize_with="::proto::serializers::write_timestamp")]
    pub timestamp: SystemTime,
    pub signatures: Vec<Signature>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReplaceDir {
    pub path: PathBuf,
    pub image: Vec<u8>,
    pub old_image: Option<Vec<u8>>,
    #[serde(deserialize_with="::proto::serializers::read_timestamp")]
    #[serde(serialize_with="::proto::serializers::write_timestamp")]
    pub timestamp: SystemTime,
    pub signatures: Vec<Signature>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppendDirAck {
    pub accepted: bool,
}


impl Request for AppendDir {
    type Response = AppendDirAck;
    fn type_name(&self) -> &'static str {
        return "AppendDir";
    }
}
