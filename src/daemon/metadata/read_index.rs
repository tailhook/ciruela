use std::io::{self, BufReader};
use std::sync::Arc;

use hex::ToHex;
use std::fs::File;

use dir_signature::v1::{Parser, ParseError as IndexError};
use ciruela::ImageId;
use index::{Index, IndexData};
use metadata::{Meta, Error};
use metadata::dir_ext::{DirExt, recover};

pub fn parse(id: &ImageId, f: File) -> Result<Index, IndexError> {
    // TODO(tailhook) verify index
    let mut parser = Parser::new(BufReader::new(f))?;
    let header = parser.get_header();
    let items = parser.iter().collect::<Result<Vec<_>, _>>()?;
    Ok(Index(Arc::new(IndexData {
        id: id.clone(),
        hash_type: header.get_hash_type(),
        block_size: header.get_block_size(),
        entries: items,
    })))
}

pub fn image_not_found(e: Error) -> Error {
    match e {
        Error::ReadMeta(_, ref e) if e.kind() == io::ErrorKind::NotFound => {
            return Error::ImageNotFound;
        }
        _ => {}
    }
    return e;
}

pub fn read(image_id: &ImageId, meta: &Meta) -> Result<Index, Error> {
    // TODO(tailhook) assert on thread name
    let hex_id = image_id.to_hex();
    let filename = format!("{}.ds1", &hex_id);
    let base = meta.base_dir.meta_sub_dir("signatures")
        .and_then(|dir| dir.meta_sub_dir(&hex_id[..2]))
        .map_err(image_not_found)?;
    let file = base.read_meta_file(&filename)
        .map_err(image_not_found)?;
    parse(image_id, file)
    .map_err(|e| Error::BadIndex(recover(&base, filename), e))
}
