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

/// Obtain two locks without deadlocks conditions
///
/// This function can starve. But usually we call it only in main loop,
/// so it doesn't matter too much.
pub fn lock2<'a, 'b, A, B>(a: &'a Mutex<A>, b: &'b Mutex<B>)
    -> (MutexGuard<'a, A>, MutexGuard<'b, B>)
{
    loop {
        {
            let aguard = a.lock();
            match b.mutex.try_lock() {
                Ok(bguard) => {
                    return (
                        aguard,
                        MutexGuard {
                            guard: bguard,
                            name: b.name,
                        },
                    );
                }
                Err(TryLockError::Poisoned(_)) => {
                    panic!("Mutex {:?} is poisoned", b.name);
                }
                Err(TryLockError::WouldBlock) => {}
            }
        }
        {
            let bguard = b.lock();
            match a.mutex.try_lock() {
                Ok(aguard) => {
                    return (
                        MutexGuard {
                            guard: aguard,
                            name: a.name,
                        },
                        bguard,
                    );
                }
                Err(TryLockError::Poisoned(_)) => {
                    panic!("Mutex {:?} is poisoned", a.name);
                }
                Err(TryLockError::WouldBlock) => {}
            }
        }
    }
}
