use std::collections::BTreeMap;

use proto::{Request, Response, Hash};
use database::signatures::State;
use virtual_path::{VPath};


#[derive(Serialize, Deserialize, Debug)]
pub struct GetBaseDir {
    pub path: VPath,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetBaseDirResponse {
    pub config_hash: Hash,
    pub keep_list_hash: Hash,
    pub dirs: BTreeMap<String, State>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BaseDirState {
    pub path: VPath,
    pub config_hash: Hash,
    pub keep_list_hash: Hash,
    pub dirs: BTreeMap<String, State>,
}

impl Request for GetBaseDir {
    type Response = GetBaseDirResponse;
    fn type_name(&self) -> &'static str {
        return "GetBaseDir";
    }
}

impl Response for GetBaseDirResponse {
    fn type_name(&self) -> &'static str {
        return "GetBaseDir";
    }
    fn static_type_name() -> &'static str {
        return "GetBaseDir";
    }
}
