use machine_id::MachineId;
use ciruela::VPath;
use ciruela::proto::Hash;


/// Maximum size of gossip packet
///
/// Larger packets allow to carry more timestamps of directories at once,
/// which efficiently makes probability of syncing new directory erlier
/// much bigger.
///
/// Making it less than MTU, makes probability of loss much smaller. On
/// other hand 1500 MTU is on WAN and slower networks. Hopefully, we are
/// efficient enough with this packet size anyway.
pub const MAX_GOSSIP_PACKET: usize = 1400;


#[derive(Serialize, Deserialize)]
pub struct Head {
    pub id: MachineId,
    /// Number of configured dirs
    pub total_dirs: usize,
    /// Number of dirs inside them, where images may be put
    pub total_basedirs: usize,
}

pub type IndividualDir = (VPath, Hash);
