use std::env;
use std::path::Path;
use std::process::Command;
use std::io::{self, Seek, SeekFrom, Read, Write};

use tempfile::Builder;
use failure::Error;


pub fn run(filename: &str, data: Vec<u8>) -> Result<Option<Vec<u8>>, Error> {
    let mut file = Builder::new()
        .suffix(filename)
        .tempfile()?;
    file.write_all(&data)?;
    _run(file.path())?;
    file.seek(SeekFrom::Start(0))?;
    let mut buf = Vec::with_capacity(data.len());
    file.read_to_end(&mut buf)?;
    if buf != data {
        return Ok(Some(buf));
    } else {
        return Ok(None);
    }
}

fn _run(file_path: &Path) -> Result<(), Error> {
    let path = if let Some(path) = env::var_os("CIRUELA_EDITOR") {
        Some(path)
    } else if let Some(path) = env::var_os("VISUAL") {
        Some(path)
    } else if let Some(path) = env::var_os("EDITOR") {
        Some(path)
    } else {
        None
    };
    if let Some(path) = path {
        let mut cmd = Command::new(&path);
        cmd.arg(file_path);
        match cmd.status() {
            Ok(s) if s.success() => return Ok(()),
            Ok(s) => bail!("{:?} exited with {}", path, s),
            Err(e) => return Err(e.into()),
        }
    } else {
        for n in &["vim", "vi", "nano", "editor"] {
            let mut cmd = Command::new(n);
            cmd.arg(file_path);
            match cmd.status() {
                Ok(s) if s.success() => return Ok(()),
                Ok(s) => return Err(format_err!("{} exited with {}", n, s)),
                Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                }
                Err(e) => return Err(e.into()),
            }
        }
        bail!("no editor found");
    }
}
