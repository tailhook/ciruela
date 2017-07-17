use std::path::{Path, PathBuf};

use metadata::Error;

use ciruela::database::signatures::State;


pub fn all_states(dir: &Path)
    -> Result<Vec<(PathBuf, State)>, Error>
{
    let res = Vec::new();
    println!("Scanning {:?}", dir);
    Ok(res)
}
