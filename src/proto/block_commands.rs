use {Hash};
use proto::{Request, Response};

#[derive(Serialize, Deserialize, Debug)]
pub struct GetBlock {
    pub hash: Hash,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetBlockResponse {
    pub data: Vec<u8>,
}

impl Request for GetBlock {
    type Response = GetBlockResponse;
    fn type_name(&self) -> &'static str {
        return "GetBlock";
    }
}

impl Response for GetBlockResponse {
    fn type_name(&self) -> &'static str {
        return "GetBlock";
    }
}
