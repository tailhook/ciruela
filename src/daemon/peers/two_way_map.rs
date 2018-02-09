use std::collections::{HashMap, HashSet, BTreeSet};
use std::collections::hash_map::Entry;

use {VPath};
use machine_id::MachineId;
use proto::Hash;

#[derive(Serialize, Debug)]
pub struct HostInfo {
    pub hash: Hash,
    pub configs: BTreeSet<VPath>,
}

#[derive(Debug)]
pub struct ConfigMap {
    // BTreeMap here is so that we can send, receive and hash it directly
    forward: HashMap<MachineId, HostInfo>,
    backward: HashMap<VPath, HashSet<MachineId>>,
}


impl ConfigMap {
    pub fn new() -> ConfigMap {
        ConfigMap {
            forward: HashMap::new(),
            backward: HashMap::new(),
        }
    }
    pub fn set(&mut self, machine: MachineId, configs: BTreeSet<VPath>) {
        let hash = Hash::for_object(&configs);
        match self.forward.entry(machine.clone()) {
            Entry::Vacant(e) => {
                for item in &configs {
                    self.backward.entry(item.clone())
                        .or_insert_with(HashSet::new)
                        .insert(machine.clone());
                }
                e.insert(HostInfo { hash, configs });
            }
            Entry::Occupied(ref e) if e.get().hash == hash => {}
            Entry::Occupied(mut e) => {
                for vpath in &e.get().configs {
                    if !configs.contains(vpath) {
                        match self.backward.entry(vpath.clone()) {
                            Entry::Vacant(_) => unreachable!(),
                            Entry::Occupied(mut e) => {
                                e.get_mut().remove(&machine);
                                if e.get().is_empty() {
                                    e.remove();
                                }
                            }
                        }
                    }
                }
                for item in &configs {
                    self.backward.entry(item.clone())
                        .or_insert_with(HashSet::new)
                        .insert(machine.clone());
                }
                e.get_mut().configs = configs;
            }
        }
    }
    pub fn get(&mut self, host: &MachineId) -> Option<&HostInfo> {
        self.forward.get(host)
    }
    pub fn by_dir(&mut self, dir: &VPath) -> Option<&HashSet<MachineId>> {
        self.backward.get(dir)
    }
    pub fn all_hosts(&self) -> &HashMap<MachineId, HostInfo> {
        &self.forward
    }
    pub fn all_dirs(&self) -> &HashMap<VPath, HashSet<MachineId>> {
        &self.backward
    }
}
