//! Functions and structures for signing uploads
use std::time::SystemTime;

use ssh_keys::PrivateKey;

use time_util::to_ms;
use index::ImageId;
use proto::{Signature, sign, SigData};
use {VPath};


/// An signed image at specified path and specified time
#[derive(Debug, Clone)]
pub struct SignedUpload {
    // TODO(tailhook) fix encapsulation when client is rewritten
    //                using clusterset
    #[doc(hidden)]
    pub path: VPath,
    #[doc(hidden)]
    pub image_id: ImageId,
    #[doc(hidden)]
    pub timestamp: SystemTime,
    #[doc(hidden)]
    pub signatures: Vec<Signature>,
}


/// Prepare a signature for upload
pub fn sign_upload(path: &VPath, image: &ImageId, timestamp: SystemTime,
    keys: &[PrivateKey])
    -> SignedUpload
{
    let signatures = sign(SigData {
        path: path.as_ref().to_str().expect("path is string"),
        image: image.as_ref(),
        timestamp: to_ms(timestamp),
    }, &keys);
    return SignedUpload {
        path: path.clone(),
        image_id: image.clone(),
        timestamp,
        signatures,
    }
}
