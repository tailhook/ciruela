use std::path::{Path, PathBuf};

use metadata::Error;

use ciruela::VPath;
use ciruela::database::signatures::State;


pub fn all_states(dir: &VPath)
    -> Result<Vec<(VPath, State)>, Error>
{
    let res = Vec::new();
    println!("Scanning {:?}", dir);
    Ok(res)
}
