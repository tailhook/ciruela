use index::{ImageId};
use {VPath};
use proto::Hash;
use machine_id::MachineId;
use mask::Mask;
use std::collections::{BTreeMap, HashSet, BTreeSet};


#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    pub machine_id: MachineId,
    pub your_config: Option<Hash>,
    pub message: Message,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    BaseDirs {
        in_progress: BTreeMap<VPath, (ImageId, Mask, bool, bool)>,
        watching: BTreeSet<VPath>,
        complete: BTreeMap<VPath, ImageId>,
        deleted: HashSet<(VPath, ImageId)>,
        base_dirs: BTreeMap<VPath, Hash>,
    },
    Downloading { path: VPath, image: ImageId, mask: Mask,
                  source: bool },
    Complete { path: VPath, image: ImageId },
    ConfigSync { paths: BTreeSet<VPath>  },
}

#[derive(Serialize, Debug)]
pub struct PacketRef<'a> {
    pub machine_id: &'a MachineId,
    pub your_config: &'a Option<Hash>,
    pub message: MessageRef<'a>,
}

#[derive(Serialize, Debug)]
#[allow(dead_code)]
pub enum MessageRef<'a> {
    BaseDirs {
        in_progress: BTreeMap<&'a VPath, (&'a ImageId, &'a Mask, bool, bool)>,
        watching: &'a BTreeSet<VPath>,
        deleted: &'a Vec<(VPath, ImageId)>,
        base_dirs: &'a BTreeMap<VPath, Hash>,
        complete: &'a BTreeMap<VPath, ImageId>,
    },
    Downloading { path: &'a VPath, image: &'a ImageId, mask: &'a Mask,
                  source: bool},
    ConfigSync { paths: &'a BTreeSet<VPath>  },
}
