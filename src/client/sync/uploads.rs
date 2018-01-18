use failure::Error;

use ciruela::signature::SignedUpload;
use sync::SyncOptions;


pub enum Upload {
    Append(SignedUpload),
    WeakAppend(SignedUpload),
    Replace(SignedUpload),
}

pub(in sync) fn prepare(opts: &SyncOptions)
    -> Result<Vec<SignedUpload>, Error>
{
    unimplemented!();
}
