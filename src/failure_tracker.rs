use std::fmt;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::hash::Hash;
use std::time::{Instant, Duration};

use abstract_ns::Name;


const RETRY_TIME: Duration = Duration::from_secs(1);
const SLOW_RETRY_TIME: Duration = Duration::from_secs(10);


pub type HostFailures = Failures<SocketAddr, DefaultPolicy>;
pub type SlowHostFailures = Failures<SocketAddr, SlowerPolicy>;
pub type DnsFailures = Failures<Name, DefaultPolicy>;

#[derive(Debug)]
pub struct DefaultPolicy;

#[derive(Debug)]
pub struct SlowerPolicy;

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
        let since = Instant::now() - entry.last;
        return since > RETRY_TIME * entry.subsequent;
    }
}

impl Policy for SlowerPolicy {
    fn can_try(&self, entry: &Failure) -> bool {
        let since = Instant::now() - entry.last;
        return since > SLOW_RETRY_TIME * entry.subsequent;
    }
}

impl<K: Clone + Eq + Hash> Failures<K, DefaultPolicy> {
    pub fn new_default() -> Failures<K, DefaultPolicy> {
        Failures {
            policy: DefaultPolicy,
            items: HashMap::new(),
        }
    }
}

impl<K: Clone + Eq + Hash> Failures<K, SlowerPolicy> {
    pub fn new_slow() -> Failures<K, SlowerPolicy> {
        Failures {
            policy: SlowerPolicy,
            items: HashMap::new(),
        }
    }
}

impl<K: Clone + Eq + Hash, P: Policy> Failures<K, P> {
    pub fn add_failure(&mut self, name: K) {
        let entry = self.items.entry(name)
            .or_insert(Failure {
                subsequent: 0,
                last: Instant::now(),
            });
        entry.subsequent = entry.subsequent.saturating_add(1);
        entry.last = Instant::now();
    }
    pub fn reset(&mut self, name: &K) {
        self.items.remove(name);
    }
    pub fn can_try(&self, name: &K) -> bool {
        self.items.get(name)
            .map(|f| self.policy.can_try(f))
            .unwrap_or(true)
    }
}
