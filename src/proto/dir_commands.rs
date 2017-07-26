use std::time::SystemTime;

use {ImageId, VPath};
use proto::{Signature, Request, Response};


#[derive(Serialize, Deserialize, Debug)]
pub struct AppendDir {
    pub path: VPath,
    pub image: ImageId,
    #[serde(deserialize_with="::proto::serializers::read_timestamp")]
    #[serde(serialize_with="::proto::serializers::write_timestamp")]
    pub timestamp: SystemTime,
    pub signatures: Vec<Signature>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReplaceDir {
    pub path: VPath,
    pub image: ImageId,
    pub old_image: Option<ImageId>,
    #[serde(deserialize_with="::proto::serializers::read_timestamp")]
    #[serde(serialize_with="::proto::serializers::write_timestamp")]
    pub timestamp: SystemTime,
    pub signatures: Vec<Signature>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppendDirAck {
    pub accepted: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReplaceDirAck {
    pub accepted: bool,
}


impl Request for AppendDir {
    type Response = AppendDirAck;
    fn type_name(&self) -> &'static str {
        return "AppendDir";
    }
}

impl Response for AppendDirAck {
    fn type_name(&self) -> &'static str {
        return "AppendDir";
    }
}

impl Request for ReplaceDir {
    type Response = ReplaceDirAck;
    fn type_name(&self) -> &'static str {
        return "ReplaceDir";
    }
}

impl Response for ReplaceDirAck {
    fn type_name(&self) -> &'static str {
        return "ReplaceDir";
    }
}
