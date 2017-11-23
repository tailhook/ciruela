use std::io::{Write};

use index::{ImageId};
use metadata::{Meta, Error};


pub fn write(id: &ImageId, data: Vec<u8>, meta: &Meta)
    -> Result<(), Error>
{
    // TODO(tailhook) assert on thread name
    let hex_id = id.to_string();
    let filename = format!("{}.ds1", &hex_id);
    let base = meta.indexes()?.ensure_dir(&hex_id[..2])?;
    base.replace_file(&filename, |mut f| f.write_all(&data))?;
    Ok(())
}
