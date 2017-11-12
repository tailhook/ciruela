use std::cmp::min;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use database::signatures::State;

use config::Directory;


#[derive(Debug)]
pub struct Image {
    pub path: PathBuf,
    pub target_state: State,
    // TODO(tailhook) real state may be different from target state:
    // * directory has never downloaded
    // * currently downloading
    // * it was delete by external process (user)
    // * it was broken
    // * it didn't pass verification
}

#[derive(Debug, PartialEq)]
pub struct Sorted {
    pub used: Vec<Image>,
    pub unused: Vec<Image>,
}

fn biggest_timestamp(img: &Image) -> SystemTime {
    img.target_state.signatures.iter()
    .map(|x| x.timestamp)
    .min()
    .unwrap_or(UNIX_EPOCH)
}

pub fn sort_out(config: &Arc<Directory>, items: Vec<Image>,
                keep_list: &Vec<PathBuf>)
    -> Sorted
{
    let mut result = Sorted {
        used: Vec::new(),
        unused: Vec::new(),
    };
    let keep_list: HashSet<_> = keep_list.iter().collect();
    if items.len() <= config.keep_min_directories {
        result.used.extend(items.into_iter());
        return result;
    }
    let mut candidates = vec![];
    let min_time = SystemTime::now() - config.keep_recent;
    for img in items.into_iter() {
        if biggest_timestamp(&img) >= min_time {
            result.used.push(img);
        } else {
            candidates.push(img);
        }
    }
    if result.used.len() > config.keep_max_directories {
        result.used.sort_by(|a, b| {
            biggest_timestamp(b).cmp(&biggest_timestamp(a))
        });
        for img in result.used.drain(config.keep_max_directories..) {
            candidates.push(img);
        }
    }
    for img in candidates.into_iter() {
        if keep_list.contains(&img.path) {
            result.used.push(img);
        } else {
            result.unused.push(img);
        }
    }
    if result.used.len() < config.keep_min_directories {
        result.unused.sort_by(|a, b| {
            biggest_timestamp(a).cmp(&biggest_timestamp(b))
        });
        let unused = result.unused.len();
        let needs = config.keep_min_directories - result.used.len();
        result.used.extend(
            result.unused.drain(unused - min(unused, needs)..));
    }
    return result;
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
    use std::time::SystemTime;
    use humantime::parse_duration;
    use rand::{thread_rng, Rng};
    use config::Directory;
    use super::{sort_out, Image, Sorted};
    use id::ImageId;
    use proto::Signature;
    use database::signatures::{State, SignatureEntry};

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
            keep_recent: parse_duration(rec).unwrap(),
        })
    }

    pub fn id() -> ImageId {
        let mut arr = vec![0u8; 32];
        thread_rng().fill_bytes(&mut arr[..]);
        return ImageId::from(arr);
    }

    pub fn sig() -> Signature {
        let mut arr = [0u8; 64];
        thread_rng().fill_bytes(&mut arr[..]);
        return Signature::SshEd25519(arr);
    }


    fn fake_state() -> State {
        State {
            image: id(),
            signatures: Vec::new(),
        }
    }

    fn state_at(from_now: &str) -> State {
        let time = SystemTime::now() - parse_duration(from_now).unwrap();
        State {
            image: id(),
            signatures: vec![SignatureEntry {
                timestamp: time,
                signature: sig(),
            }],
        }
    }
    #[test]
    fn test_zero() {
        assert_eq!(sort_out(&cfg(1, 2, "1 day"), vec![], &vec![]),
            Sorted {
                used: vec![],
                unused: vec![],
            });
    }

    #[test]
    fn test_few() {
        assert_eq!(sort_out(&cfg(1, 2, "1 day"), vec![
            Image {
                path: PathBuf::from("t1"),
                target_state: fake_state(),
            },
        ], &vec![]),
            Sorted {
                used: vec![
                    Image {
                        path: PathBuf::from("t1"),
                        target_state: fake_state(),
                    }
                ],
                unused: vec![],
            });
    }

    #[test]
    fn test_recent() {
        assert_eq!(sort_out(&cfg(1, 100, "1 day"), vec![
            Image {
                path: PathBuf::from("t1"),
                target_state: state_at("1 hour"),
            },
            Image {
                path: PathBuf::from("t2"),
                target_state: state_at("1 week"),
            },
            Image {
                path: PathBuf::from("t3"),
                target_state: state_at("1 sec"),
            },
        ], &vec![]),
            Sorted {
                used: vec![
                    Image {
                        path: PathBuf::from("t1"),
                        target_state: fake_state(),
                    },
                    Image {
                        path: PathBuf::from("t3"),
                        target_state: fake_state(),
                    },
                ],
                unused: vec![
                    Image {
                        path: PathBuf::from("t2"),
                        target_state: fake_state(),
                    },
                ],
            });
    }

    #[test]
    fn test_few_recent() {
        assert_eq!(sort_out(&cfg(2, 100, "1 min"), vec![
            Image {
                path: PathBuf::from("t1"),
                target_state: state_at("1 hour"),
            },
            Image {
                path: PathBuf::from("t2"),
                target_state: state_at("1 week"),
            },
            Image {
                path: PathBuf::from("t3"),
                target_state: state_at("1 sec"),
            },
        ], &vec![]),
            Sorted {
                used: vec![
                    Image {
                        path: PathBuf::from("t3"),
                        target_state: fake_state(),
                    },
                    Image {
                        path: PathBuf::from("t1"),
                        target_state: fake_state(),
                    },
                ],
                unused: vec![
                    Image {
                        path: PathBuf::from("t2"),
                        target_state: fake_state(),
                    },
                ],
            });
    }

    #[test]
    fn test_more_than_max() {
        assert_eq!(sort_out(&cfg(1, 2, "1 day"), vec![
            Image {
                path: PathBuf::from("t1"),
                target_state: state_at("1 week"),
            },
            Image {
                path: PathBuf::from("t2"),
                target_state: state_at("1 hour"),
            },
            Image {
                path: PathBuf::from("t3"),
                target_state: state_at("30 min"),
            },
            Image {
                path: PathBuf::from("t4"),
                target_state: state_at("2 min"),
            },
            Image {
                path: PathBuf::from("t5"),
                target_state: state_at("1 year"),
            },
        ], &vec![]),
            Sorted {
                used: vec![
                    Image {
                        path: PathBuf::from("t4"),
                        target_state: fake_state(),
                    },
                    Image {
                        path: PathBuf::from("t3"),
                        target_state: fake_state(),
                    },
                ],
                unused: vec![
                    Image {
                        path: PathBuf::from("t1"),
                        target_state: fake_state(),
                    },
                    Image {
                        path: PathBuf::from("t5"),
                        target_state: fake_state(),
                    },
                    Image {
                        path: PathBuf::from("t2"),
                        target_state: fake_state(),
                    },
                ],
            });
    }

    #[test]
    fn test_keep_list() {
        assert_eq!(sort_out(&cfg(1, 2, "1 day"), vec![
            Image {
                path: PathBuf::from("t1"),
                target_state: state_at("1 week"),
            },
            Image {
                path: PathBuf::from("t2"),
                target_state: state_at("1 hour"),
            },
            Image {
                path: PathBuf::from("t3"),
                target_state: state_at("30 min"),
            },
            Image {
                path: PathBuf::from("t4"),
                target_state: state_at("2 min"),
            },
            Image {
                path: PathBuf::from("t5"),
                target_state: state_at("1 year"),
            },
        ], &vec![PathBuf::from("t5")]),
            Sorted {
                used: vec![
                    Image {
                        path: PathBuf::from("t4"),
                        target_state: fake_state(),
                    },
                    Image {
                        path: PathBuf::from("t3"),
                        target_state: fake_state(),
                    },
                    Image {
                        path: PathBuf::from("t5"),
                        target_state: fake_state(),
                    },
                ],
                unused: vec![
                    Image {
                        path: PathBuf::from("t1"),
                        target_state: fake_state(),
                    },
                    Image {
                        path: PathBuf::from("t2"),
                        target_state: fake_state(),
                    },
                ],
            });
    }
}
