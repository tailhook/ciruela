use std::path::PathBuf;
use std::time::SystemTime;

use proto::{Signature};


pub struct AppendDir {
    pub path: PathBuf,
    pub image: Vec<u8>,
    pub timestamp: SystemTime,
    pub signatures: Vec<Signature>,
}

pub struct ReplaceDir {
    pub path: PathBuf,
    pub image: Vec<u8>,
    pub old_image: Option<Vec<u8>>,
    pub timestamp: SystemTime,
    pub signatures: Vec<Signature>,
}

