use std::io::{self, BufReader, BufRead};
use std::sync::Arc;

use hex::ToHex;
use std::fs::File;

use ciruela::ImageId;
use index::{Index, IndexData};
use metadata::{Meta, Error};
use metadata::dir_ext::{DirExt, recover};

pub fn index_not_found(e: Error) -> Error {
    match e {
        Error::OpenMeta(_, ref e)
        | Error::ReadMeta(_, ref e)
        if e.kind() == io::ErrorKind::NotFound => {
            return Error::IndexNotFound;
        }
        _ => {}
    }
    return e;
}

pub fn read(image_id: &ImageId, meta: &Meta) -> Result<Index, Error> {
    // TODO(tailhook) assert on thread name
    let hex_id = image_id.to_hex();
    let filename = format!("{}.ds1", &hex_id);
    let base = meta.base_dir.meta_sub_dir("indexes")
        .and_then(|dir| dir.meta_sub_dir(&hex_id[..2]))
        .map_err(index_not_found)?;
    let file = base.read_meta_file(&filename)
        .map_err(index_not_found)?;
    Index::parse(image_id, BufReader::new(file))
    .map_err(|e| Error::BadIndex(recover(&base, filename), e))
}
