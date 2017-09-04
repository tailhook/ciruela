use std::fmt;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::hash::Hash;
use std::time::{Instant, Duration};


const RETRY_TIME: u64 = 1000;


pub type HostFailures = Failures<SocketAddr, DefaultPolicy>;

#[derive(Debug)]
pub struct DefaultPolicy;

#[derive(Debug)]
pub struct Failures<K: Eq + Hash, P> {
    items: HashMap<K, Failure>,
    policy: P,
}

#[derive(Debug)]
pub struct Failure {
    subsequent: u32,
    last: Instant,
}

pub trait Policy: fmt::Debug {
    fn can_try(&self, entry: &Failure) -> bool;
}

impl Policy for DefaultPolicy {
    fn can_try(&self, entry: &Failure) -> bool {
        let retry_time = Duration::from_millis(RETRY_TIME);
        let since = Instant::now() - entry.last;
        return since > retry_time * entry.subsequent;
    }
}
impl<K: Copy + Eq + Hash> Failures<K, DefaultPolicy> {
    pub fn new_default() -> Failures<K, DefaultPolicy> {
        Failures {
            policy: DefaultPolicy,
            items: HashMap::new(),
        }
    }
}

impl<K: Copy + Eq + Hash, P: Policy> Failures<K, P> {
    pub fn add_failure(&mut self, name: K) {
        let mut entry = self.items.entry(name)
            .or_insert(Failure {
                subsequent: 0,
                last: Instant::now(),
            });
        entry.subsequent.saturating_add(1);
        entry.last = Instant::now();
    }
    pub fn reset(&mut self, name: K) {
        self.items.remove(&name);
    }
    pub fn can_try(&self, name: K) -> bool {
        self.items.get(&name)
            .map(|f| self.policy.can_try(f))
            .unwrap_or(true)
    }
}
