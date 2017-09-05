use std::ops::{Deref, DerefMut};
use std::sync::{Mutex as StdMutex, MutexGuard as StdGuard};
use std::sync::{TryLockError};


#[derive(Debug)]
pub struct Mutex<T> {
    mutex: StdMutex<T>,
    name: &'static str,
}

#[derive(Debug)]
pub struct MutexGuard<'a, T: 'static> {
    guard: StdGuard<'a, T>,
    name: &'static str,
}

impl<T: 'static> Mutex<T> {
    pub fn new(value: T, name: &'static str) -> Mutex<T> {
        trace!("Created mutex {:?}", name);
        Mutex {
            mutex: StdMutex::new(value),
            name: name,
        }
    }
    pub fn lock(&self) -> MutexGuard<T> {
        trace!("Locking {:?}", self.name);
        MutexGuard {
            guard: match self.mutex.lock() {
                Ok(x) => x,
                Err(_) => {
                    panic!("Mutex {:?} is poisoned", self.name);
                }
            },
            name: self.name
        }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        trace!("Unlocking {:?}", self.name)
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &*self.guard
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut*self.guard
    }
}
