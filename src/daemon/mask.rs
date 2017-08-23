use std::fmt;

use atomic::{Atomic, Ordering};
use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};

#[derive(Clone, Copy)]
pub struct Mask(u16);

pub struct AtomicMask(Atomic<u16>);


impl Mask {
    pub fn new() -> Mask {
        Mask(0)
    }
}

impl AtomicMask {
    pub fn new() -> AtomicMask {
        AtomicMask(Atomic::new(0))
    }
}

impl Into<Mask> for AtomicMask {
    fn into(self) -> Mask {
        Mask(self.0.load(Ordering::SeqCst))
    }
}

impl Into<AtomicMask> for Mask {
    fn into(self) -> AtomicMask {
        AtomicMask(Atomic::new(self.0))
    }
}

impl fmt::Debug for Mask {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:16b}", self.0)
    }
}

impl fmt::Debug for AtomicMask {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:16b}", self.0.load(Ordering::SeqCst))
    }
}

impl<'a> Deserialize<'a> for Mask {
    fn deserialize<D: Deserializer<'a>>(d: D) -> Result<Mask, D::Error> {
        Ok(Mask(u16::deserialize(d)?))
    }
}

impl Serialize for Mask {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'a> Deserialize<'a> for AtomicMask {
    fn deserialize<D: Deserializer<'a>>(d: D)
        -> Result<AtomicMask, D::Error>
    {
        Ok(AtomicMask(Atomic::new(u16::deserialize(d)?)))
    }
}

impl Serialize for AtomicMask {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.load(Ordering::SeqCst).serialize(s)
    }
}

impl Clone for AtomicMask {
    fn clone(&self) -> AtomicMask {
        AtomicMask(Atomic::new(self.0.load(Ordering::SeqCst)))
    }
}
