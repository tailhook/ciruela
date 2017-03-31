use std::io::BufRead;
use std::sync::{Arc, Weak};
use std::ops::Deref;


use dir_signature::v1::{Entry};
use dir_signature::HashType;
use dir_signature::v1::{Parser, ParseError as IndexError};

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

    pub fn parse<F: BufRead>(id: &ImageId, f: F) -> Result<Index, IndexError> {
        // TODO(tailhook) verify index
        let mut parser = Parser::new(f)?;
        let header = parser.get_header();
        let items = parser.iter().collect::<Result<Vec<_>, _>>()?;
        Ok(Index(Arc::new(IndexData {
            id: id.clone(),
            hash_type: header.get_hash_type(),
            block_size: header.get_block_size(),
            entries: items,
        })))
    }
}
