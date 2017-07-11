use std::sync::Arc;
use std::os::unix::fs::{PermissionsExt, symlink};

use disk::dir::{ensure_path, recover_path};
use disk::error::Error;
use disk::public::Image;

use dir_signature::v1::Entry::*;


pub fn commit_image(image: Arc<Image>) -> Result<(), Error> {
    debug!("Preparing commit {:?}", image.image_name);
    // TODO(tailhook) maybe throttle
    let mut dir = None;
    for entry in &image.index.entries {
        match *entry {
            Dir(ref path) => {
                dir = Some((ensure_path(&image.temporary, path)?, path));
                // TODO(tailhook) check permissions?
            }
            File { ref path, exe, size, ref hashes } => {
                let &(ref dir, ref dpath) = dir.as_ref().unwrap();
                // Assuming dir-signature can't yield records out of order..
                debug_assert!(*dpath == path.parent().unwrap());
                // ... and having filenames
                let filename = path.file_name().expect("file has filename");
                let mut file = dir.open_file(filename)
                    .map_err(|e| Error::ReadFile(
                        recover_path(dir, filename), e))?;
                let ok = hashes.check_file(&mut file)
                    .map_err(|e| Error::ReadFile(
                        recover_path(dir, filename), e))?;
                if !ok {
                    return Err(
                        Error::Checksum(recover_path(dir, filename)));
                }
                if exe {
                    file.set_permissions(PermissionsExt::from_mode(0o755))
                    .map_err(|e| Error::SetPermissions(
                        recover_path(dir, filename), e))?;
                }
            }
            Link(ref link, ref dest) => {
                let &(ref dir, ref dpath) = dir.as_ref().unwrap();
                // Assuming dir-signature can't yield records out of order..
                debug_assert!(*dpath == link.parent().unwrap());
                let filename = link.file_name().expect("symlink has filename");
                dir.symlink(filename, dest.as_os_str())
                    .map_err(|e| Error::CreateSymlink(
                        recover_path(dir, filename), e))?;
            }
        }
    }
    // TODO(tailhook) check extra files and directories
    debug!("Commiting {:?}", image.image_name);

    image.parent.local_rename(&image.temporary_name, &image.image_name)
        .map_err(|e| Error::Commit(
            recover_path(&image.parent, &image.image_name), e))?;

    Ok(())
}
