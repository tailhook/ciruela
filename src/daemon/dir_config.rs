use std::sync::Arc;
use std::path::Path;

use config::{Directory};


pub struct DirConfig<'a> {
    pub virtual_path: &'a Path,
    pub base: &'a Path,
    pub parent: &'a Path,
    pub image_name: &'a str,
    pub config: &'a Arc<Directory>,
}
