mod commit;
mod dir;
mod error;
mod public;

pub use self::public::{Disk, Image, start};
pub use self::error::Error;

use metrics::{List, Metric};

pub struct Init {
}

pub fn metrics() -> List {
    let hlinks = "disk.hardlinks";
    let comm = "disk.committed";
    vec![
        (Metric(hlinks, "files"), &*self::public::HARDLINKED_FILES),
        (Metric(hlinks, "bytes"), &*self::public::HARDLINKED_BYTES),
        (Metric(comm, "images"), &*self::commit::IMAGES),
        (Metric(comm, "bytes"), &*self::commit::BYTES),
        (Metric(comm, "blocks"), &*self::commit::BLOCKS),
        (Metric(comm, "paths"), &*self::commit::PATHS),
    ]
}
