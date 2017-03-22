use dir_signature::v1::{Entry};
use dir_signature::HashType;

use ciruela::ImageId;


pub struct Index {
    pub id: ImageId,
    pub hash_type: HashType,
    pub block_size: u64,
    pub entries: Vec<Entry>,
}
