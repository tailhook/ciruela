use openat::Dir;
use quick_error::ResultExt;

use ciruela::proto::{AppendDir, AppendDirAck};
use metadata::{Meta, Error};


pub fn start(params: AppendDir, meta: &Meta)
    -> Result<AppendDirAck, Error>
{
    //let ref path = meta.config.db_dir;
    unimplemented!();
}
