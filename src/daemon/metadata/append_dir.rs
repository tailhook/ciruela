use std::path::Path;


use ciruela::proto::{AppendDir, AppendDirAck};
use metadata::{Meta, Error, find_config_dir};


pub fn check_path(path: &Path) -> Result<&Path, Error> {
    use std::path::Component::Normal;
    let result = path.strip_prefix("/").map_err(|_| Error::InvalidPath)?;
    for cmp in result.components() {
        if let Normal(_) = cmp {
            continue;
        }
        return Err(Error::InvalidPath);
    }
    Ok(result)
}


pub fn start(params: AppendDir, meta: &Meta)
    -> Result<AppendDirAck, Error>
{
    let path = check_path(&params.path)?;

    let dir = find_config_dir(&meta.config, path)?;
    info!("Directory {:?} has base {:?} and suffix {:?}",
        params.path, dir.base, dir.suffix);
    unimplemented!();
}
