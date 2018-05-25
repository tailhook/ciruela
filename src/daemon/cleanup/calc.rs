use std::cmp::min;
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use database::signatures::State;

use config::Directory;

#[derive(Debug, PartialEq)]
pub struct Sorted<T> {
    pub used: Vec<T>,
    pub unused: Vec<T>,
}

fn biggest_timestamp(state: &State) -> SystemTime {
    state.signatures.iter()
    .map(|x| x.timestamp)
    .min()
    .unwrap_or(UNIX_EPOCH)
}

pub fn sort_out<T>(config: &Arc<Directory>, items: Vec<(T, State)>,
                keep_list: &Vec<T>)
    -> Sorted<(T, State)>
    where T: PartialEq + Eq + Hash,
{
    let mut used = Vec::new();
    let mut unused = Vec::new();
    let keep_list: HashSet<_> = keep_list.iter().collect();
    if items.len() <= config.keep_min_directories {
        return Sorted {
            used: items,
            unused: vec![],
        }
    }
    let mut candidates = vec![];
    let min_time = SystemTime::now() - config.keep_recent;
    for (name, state) in items.into_iter() {
        if biggest_timestamp(&state) >= min_time {
            used.push((name, state));
        } else {
            candidates.push((name, state));
        }
    }
    if used.len() > config.keep_max_directories {
        used.sort_by(|&(_, ref a), &(_, ref b)| {
            biggest_timestamp(b).cmp(&biggest_timestamp(a))
        });
        for pair in used.drain(config.keep_max_directories..) {
            candidates.push(pair);
        }
    }
    for (name, state) in candidates.into_iter() {
        if keep_list.contains(&name) {
            used.push((name, state));
        } else {
            unused.push((name, state));
        }
    }
    if used.len() < config.keep_min_directories {
        unused.sort_by(|&(_, ref a), &(_, ref b)| {
            biggest_timestamp(a).cmp(&biggest_timestamp(b))
        });
        let unused_len = unused.len();
        let needs = config.keep_min_directories - used.len();
        used.extend(unused.drain(unused_len - min(unused_len, needs)..));
    }
    return Sorted {
        used: used,
        unused: unused,
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::SystemTime;
    use humantime::parse_duration;
    use rand::{thread_rng, RngCore};
    use config::Directory;
    use super::{sort_out, Sorted};
    use index::{ImageId};
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

    fn simple_sort(config: &Arc<Directory>, items: Vec<(u32, State)>,
                    keep_list: &Vec<u32>)
        -> Sorted<u32>
    {
        let r = sort_out(config, items, keep_list);
        return Sorted {
            used: r.used.into_iter().map(|(x, _)| x).collect(),
            unused: r.unused.into_iter().map(|(x, _)| x).collect(),
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
        assert_eq!(simple_sort(&cfg(1, 2, "1 day"), vec![], &vec![]),
            Sorted {
                used: vec![],
                unused: vec![],
            });
    }

    #[test]
    fn test_few() {
        assert_eq!(simple_sort(&cfg(1, 2, "1 day"), vec![
            (1, fake_state()),
        ], &vec![]),
            Sorted {
                used: vec![1],
                unused: vec![],
            });
    }

    #[test]
    fn test_recent() {
        assert_eq!(simple_sort(&cfg(1, 100, "1 day"), vec![
            (1, state_at("1 hour")),
            (2, state_at("1 week")),
            (3, state_at("1 sec")),
        ], &vec![]),
            Sorted {
                used: vec![1, 3],
                unused: vec![2],
            });
    }

    #[test]
    fn test_few_recent() {
        assert_eq!(simple_sort(&cfg(2, 100, "1 min"), vec![
            (1, state_at("1 hour")),
            (2, state_at("1 week")),
            (3, state_at("1 sec")),
        ], &vec![]),
            Sorted {
                used: vec![3, 1],
                unused: vec![2],
            });
    }

    #[test]
    fn test_more_than_max() {
        assert_eq!(simple_sort(&cfg(1, 2, "1 day"), vec![
            (1, state_at("1 week")),
            (2, state_at("1 hour")),
            (3, state_at("30 min")),
            (4, state_at("2 min")),
            (5, state_at("1 year")),
        ], &vec![]),
            Sorted {
                used: vec![4, 3],
                unused: vec![1, 5, 2],
            });
    }

    #[test]
    fn test_keep_list() {
        assert_eq!(simple_sort(&cfg(1, 2, "1 day"), vec![
            (1, state_at("1 week")),
            (2, state_at("1 hour")),
            (3, state_at("30 min")),
            (4, state_at("2 min")),
            (5, state_at("1 year")),
        ], &vec![5]),
            Sorted {
                used: vec![4, 3, 5],
                unused: vec![1, 2],
            });
    }
}
