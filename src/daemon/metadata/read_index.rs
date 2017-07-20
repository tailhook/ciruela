use std::io::{self, BufReader};

use hex::ToHex;
use std::fs::File;

use ciruela::ImageId;
use index::Index;
use metadata::{Meta, Error};


pub fn read(image_id: &ImageId, meta: &Meta) -> Result<Index, Error> {
    // TODO(tailhook) assert on thread name
    let hex_id = image_id.to_hex();
    let filename = format!("{}.ds1", &hex_id);
    let base = meta.indexes()?
        .dir_if_exists(&hex_id[..2])?
        .ok_or(Error::IndexNotFound)?;
    let index = base.read_file(&filename, |f| {
            Index::parse(image_id, BufReader::new(f))
        })?
        .ok_or(Error::IndexNotFound)?;
    Ok(index)
}
