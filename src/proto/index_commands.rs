use std::fmt;

use serde_bytes;

use proto::Notification;
use {ImageId};
use proto::{Request, Response};


#[derive(Serialize, Deserialize, Debug)]
pub struct PublishImage {
    pub image_id: ImageId,
}

impl Notification for PublishImage {
    fn type_name(&self) -> &'static str {
        "PublishImage"
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetIndex {
    pub id: ImageId,
}

#[derive(Serialize, Deserialize)]
pub struct GetIndexResponse {
    #[serde(with="serde_bytes")]
    pub data: Vec<u8>,
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
}

impl fmt::Debug for GetIndexResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("GetIndexResponse")
        .field("data", &format!("<{} bytes>", self.data.len()))
        .finish()
    }
}
