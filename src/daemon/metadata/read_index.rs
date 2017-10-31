use std::io::{Read, BufReader};
use std::fs::File;

use hex::ToHex;

use ciruela::ImageId;
use index::IndexData;
use metadata::{Meta, Error};
use dir_signature::v1::Parser;


pub fn read_bytes(image_id: &ImageId, meta: &Meta) -> Result<Vec<u8>, Error> {
    // TODO(tailhook) assert on thread name
    let hex_id = image_id.to_hex();
    let filename = format!("{}.ds1", &hex_id);
    let base = meta.indexes()?
        .dir_if_exists(&hex_id[..2])?
        .ok_or(Error::IndexNotFound)?;
    let mut buf = Vec::with_capacity(4096);
    base.read_file(&filename, |mut f| f.read_to_end(&mut buf))?
        .ok_or(Error::IndexNotFound)?;
    Ok(buf)
}

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

pub fn open(image_id: &ImageId, meta: &Meta)
    -> Result<Parser<BufReader<File>>, Error>
{
    let hex_id = image_id.to_hex();
    let filename = format!("{}.ds1", &hex_id);
    let base = meta.indexes()?
        .dir_if_exists(&hex_id[..2])?
        .ok_or(Error::IndexNotFound)?;
    let parser = Parser::new(BufReader::new(base.open_file(&filename)?
        .ok_or(Error::IndexNotFound)?))?;
    Ok(parser)
}
