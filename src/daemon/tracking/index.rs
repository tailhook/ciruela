use std::sync::{Arc, Weak};
use std::ops::Deref;


use dir_signature::v1::{Entry};
use dir_signature::HashType;

use ciruela::ImageId;


#[derive(Clone)]
pub struct Index(pub Arc<IndexData>);

pub struct IndexData {
    pub id: ImageId,
    pub hash_type: HashType,
    pub block_size: u64,
    pub entries: Vec<Entry>,
}


impl Deref for Index {
    type Target = IndexData;
    fn deref(&self) -> &IndexData {
        &self.0
    }
}

impl Index {
    pub fn weak(&self) -> Weak<IndexData> {
        Arc::downgrade(&self.0)
    }
}
