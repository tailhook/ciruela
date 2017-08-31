use ciruela::{ImageId, VPath, Hash};
use machine_id::MachineId;
use mask::Mask;
use std::collections::BTreeMap;


#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    pub machine_id: MachineId,
    pub message: Message,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    BaseDirs {
        in_progress: BTreeMap<VPath, (ImageId, Mask)>,
        base_dirs: BTreeMap<VPath, Hash>,
    },
    Downloading { path: VPath, image: ImageId, mask: Mask },
}

#[derive(Serialize, Debug)]
pub struct PacketRef<'a> {
    pub machine_id: &'a MachineId,
    pub message: MessageRef<'a>,
}

#[derive(Serialize, Debug)]
#[allow(dead_code)]
pub enum MessageRef<'a> {
    BaseDirs {
        in_progress: &'a BTreeMap<VPath, (ImageId, Mask)>,
        base_dirs: &'a BTreeMap<VPath, Hash>,
    },
    Downloading { path: &'a VPath, image: &'a ImageId, mask: &'a Mask },
}
