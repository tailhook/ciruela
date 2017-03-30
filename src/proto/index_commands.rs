use proto::Notification;

use proto::{Request, Response};

#[derive(Serialize, Deserialize, Debug)]
pub struct PublishIndex {
    pub image_id: Vec<u8>,
}

impl Notification for PublishIndex {
    fn type_name(&self) -> &'static str {
        "PublishIndex"
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetIndex {
    pub id: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetIndexResponse {
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
