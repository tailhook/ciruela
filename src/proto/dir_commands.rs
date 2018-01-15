use std::collections::HashMap;
use std::time::SystemTime;

use index::ImageId;
use machine_id::MachineId;
use proto::{Signature, SigData, Request, Response};
use serialize::timestamp;
// use signature::SignedUpload;
use time_util::to_ms;
use {VPath};


#[derive(Serialize, Deserialize, Debug)]
pub struct AppendDir {
    pub path: VPath,
    pub image: ImageId,
    #[serde(with="timestamp")]
    pub timestamp: SystemTime,
    pub signatures: Vec<Signature>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReplaceDir {
    pub path: VPath,
    pub image: ImageId,
    pub old_image: Option<ImageId>,
    #[serde(with="timestamp")]
    pub timestamp: SystemTime,
    pub signatures: Vec<Signature>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppendDirAck {
    pub accepted: bool,
    pub reject_reason: Option<String>,
    pub hosts: HashMap<MachineId, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReplaceDirAck {
    pub accepted: bool,
    pub reject_reason: Option<String>,
    pub hosts: HashMap<MachineId, String>,
}

impl AppendDir {
    pub fn sig_data(&self) -> SigData {
        SigData {
            path: self.path.as_ref().to_str().expect("path is string"),
            image: self.image.as_ref(),
            timestamp: to_ms(self.timestamp),
        }
    }
    /*
    pub fn new(up: &SignedUpload) -> AppendDir {
        AppendDir {
            image: up.image_id.clone(),
            timestamp: up.timestamp,
            signatures: up.signatures.clone(),
            path: up.path.clone(),
        }
    }
    */
}

impl ReplaceDir {
    pub fn sig_data(&self) -> SigData {
        SigData {
            path: self.path.as_ref().to_str().expect("path is string"),
            image: self.image.as_ref(),
            timestamp: to_ms(self.timestamp),
        }
    }
    /*
    pub fn new(up: &SignedUpload, old: Option<&ImageId>) -> ReplaceDir {
        ReplaceDir {
            image: up.image_id.clone(),
            timestamp: up.timestamp,
            signatures: up.signatures.clone(),
            path: up.path.clone(),
            old_image: old.map(|x| x.clone()),
        }
    }
    */
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
    fn static_type_name() -> &'static str {
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
    fn static_type_name() -> &'static str {
        return "ReplaceDir";
    }
}
