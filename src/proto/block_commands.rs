use proto::{Request, Response};
use std::fmt;
use {Hash};

use serde_bytes;

#[derive(Serialize, Deserialize, Debug)]
pub struct GetBlock {
    pub hash: Hash,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlockResponse {
    #[serde(with="serde_bytes")]
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

impl fmt::Debug for GetBlockResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("GetBlockResponse")
        .field("data", &format!("<{} bytes>", self.data.len()))
        .finish()
    }
}
