use std::io::BufRead;
use std::ops::Deref;


use dir_signature::v1::{Entry};
use dir_signature::HashType;
use dir_signature::v1::{Parser, ParseError as IndexError};

use ciruela::ImageId;


pub struct IndexData {
    pub id: ImageId,
    pub hash_type: HashType,
    pub block_size: u64,
    pub entries: Vec<Entry>,
    pub bytes_total: u64,
    pub blocks_total: u64,
}


fn bytes(e: &Entry) -> u64 {
    match *e {
        Entry::File { size, ..} => size,
        _ => 0,
    }
}

fn blocks(e: &Entry, block_size: u64) -> u64 {
    match *e {
        Entry::File { size, ..} => (size + block_size-1) / block_size,
        _ => 0,
    }
}

impl IndexData {
    pub fn parse<F: BufRead>(id: &ImageId, f: F)
        -> Result<IndexData, IndexError>
    {
        // TODO(tailhook) verify index
        let mut parser = Parser::new(f)?;
        let header = parser.get_header();
        let items = parser.iter().collect::<Result<Vec<_>, _>>()?;
        Ok(IndexData {
            id: id.clone(),
            hash_type: header.get_hash_type(),
            block_size: header.get_block_size(),
            bytes_total: items.iter()
                .map(bytes)
                .sum(),
            blocks_total: items.iter()
                .map(|x| blocks(x, header.get_block_size()))
                .sum(),
            entries: items,
        })
    }
}
