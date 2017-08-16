use std::io::{BufReader};

use hex::ToHex;

use ciruela::ImageId;
use index::IndexData;
use metadata::{Meta, Error};


pub fn read(image_id: &ImageId, meta: &Meta) -> Result<IndexData, Error> {
    // TODO(tailhook) assert on thread name
    let hex_id = image_id.to_hex();
    let filename = format!("{}.ds1", &hex_id);
    let base = meta.indexes()?
        .dir_if_exists(&hex_id[..2])?
        .ok_or(Error::IndexNotFound)?;
    let index = base.read_file(&filename, |f| {
            IndexData::parse(image_id, BufReader::new(f))
        })?
        .ok_or(Error::IndexNotFound)?;
    Ok(index)
}
