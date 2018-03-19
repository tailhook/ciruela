use std::fmt;
use std::collections::HashMap;

use serde_bytes;

use index::ImageId;
use machine_id::{MachineId};
use proto::request::Notification;
use proto::{Request, Response};
use {VPath};


#[derive(Serialize, Deserialize, Debug)]
pub struct PublishImage {
    pub id: ImageId,
}

impl Notification for PublishImage {
    fn type_name(&self) -> &'static str {
        "PublishImage"
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReceivedImage {
    pub id: ImageId,
    pub path: VPath,
    pub machine_id: MachineId,
    pub hostname: String,
    pub forwarded: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AbortedImage {
    pub id: ImageId,
    pub path: VPath,
    pub machine_id: MachineId,
    pub hostname: String,
    pub forwarded: bool,
    pub reason: String,
}

impl Notification for ReceivedImage {
    fn type_name(&self) -> &'static str {
        "ReceivedImage"
    }
}

impl Notification for AbortedImage {
    fn type_name(&self) -> &'static str {
        "AbortedImage"
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetIndex {
    pub id: ImageId,
    pub hint: Option<VPath>,
}

#[derive(Serialize, Deserialize)]
pub struct GetIndexResponse {
    #[serde(with="serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetIndexAt {
    pub path: VPath,
}

#[derive(Serialize, Deserialize)]
pub struct GetIndexAtResponse {
    pub data: Option<serde_bytes::ByteBuf>,
    #[serde(default)]
    pub hosts: HashMap<MachineId, String>,
}


impl Request for GetIndex {
    type Response = GetIndexResponse;
    fn type_name(&self) -> &'static str {
        return "GetIndex";
    }
}

impl Response for GetIndexResponse {
    fn type_name(&self) -> &'static str {
        return "GetIndex";
    }
    fn static_type_name() -> &'static str {
        return "GetIndex";
    }
}

impl fmt::Debug for GetIndexResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("GetIndexResponse")
        .field("data", &format!("<{} bytes>", self.data.len()))
        .finish()
    }
}

impl Request for GetIndexAt {
    type Response = GetIndexAtResponse;
    fn type_name(&self) -> &'static str {
        return "GetIndexAt";
    }
}

impl Response for GetIndexAtResponse {
    fn type_name(&self) -> &'static str {
        return "GetIndexAt";
    }
    fn static_type_name() -> &'static str {
        return "GetIndexAt";
    }
}

impl fmt::Debug for GetIndexAtResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("GetIndexAtResponse")
        .field("data", &match self.data {
            Some(ref d) => format!("<{} bytes>", d.len()),
            None => format!("<no data>"),
        })
        .field("hosts", &self.hosts)
        .finish()
    }
}
