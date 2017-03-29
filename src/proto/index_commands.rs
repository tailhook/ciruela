use proto::NotificationTrait;

use proto::{RequestTrait};

#[derive(Serialize, Deserialize, Debug)]
pub struct PublishIndex {
    pub image_id: Vec<u8>,
}

impl NotificationTrait for PublishIndex {
    fn type_name(&self) -> &'static str {
        "PublishIndex"
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetIndex {
    id: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetIndexResponse {
    pub data: Vec<u8>,
}


impl RequestTrait for GetIndex {
    type Response = GetIndexResponse;
    fn type_name(&self) -> &'static str {
        return "GetIndex";
    }
}
