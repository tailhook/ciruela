use std::sync::Arc;
use std::path::PathBuf;

use ciruela::database::signatures::State;

use config::Directory;


#[derive(Debug)]
pub struct Image {
    pub path: PathBuf,
    pub target_state: State,
    // TODO(tailhook) real state may be different from target state:
    // * directory has never downloaded
    // * it was delete by external process (user)
    // * it was broken
    // * it didn't pass verification
}

#[derive(Debug, PartialEq)]
pub struct Sorted {
    pub used: Vec<Image>,
    pub unused: Vec<Image>,
}

pub fn sort_out(config: &Arc<Directory>, items: Vec<Image>,
                keep_list: Vec<PathBuf>)
    -> Sorted
{
    let mut result = Sorted {
        used: Vec::new(),
        unused: Vec::new(),
    };
    if items.len() <= config.keep_min_directories {
        result.used.extend(items.into_iter());
        return result;
    }
    unimplemented!();
}

impl PartialEq for Image {
    fn eq(&self, other: &Image) -> bool {
        self.path == other.path
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use std::sync::Arc;
    use config::Directory;
    use super::{sort_out, Image, State, Sorted};
    use humantime::parse_duration;
    use quire::De;

    fn cfg(min: usize, max: usize, rec: &str) -> Arc<Directory> {
        Arc::new(Directory {
            directory: PathBuf::from("<nowhere>"),
            append_only: true,  // doesn't matter
            num_levels: 1,
            upload_keys: Vec::new(),
            download_keys: Vec::new(),
            auto_clean: true,
            keep_list_file: None,  // doesn't matter
            keep_min_directories: min,
            keep_max_directories: max,
            keep_recent: De::from(parse_duration(rec).unwrap()),
        })
    }

    #[test]
    fn test_zero() {
        assert_eq!(sort_out(&cfg(1, 2, "1 day"), vec![], vec![]),
            Sorted {
                used: vec![],
                unused: vec![],
            });
    }

}
