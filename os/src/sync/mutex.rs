//! Mutex (spin-like and blocking(sleep))

use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::{current_process, current_task, wakeup_task};
use alloc::{collections::VecDeque, sync::Arc};
use crate::sync::{ SyncRes, DEAD_LOCK };

/// Mutex trait
pub trait Mutex: Sync + Send + SyncRes {
    /// Lock the mutex
    fn lock(&self) -> i32;
    /// Unlock the mutex
    fn unlock(&self);
}

/// Spinlock Mutex struct
pub struct MutexSpin {
    locked: UPSafeCell<bool>,
    resid: u32,
}

impl MutexSpin {
    /// Create a new spinlock mutex
    pub fn new() -> Self {
        let curproc = current_process();
        let mut resmon = curproc.resmon.exclusive_access();
        Self {
            locked: unsafe { UPSafeCell::new(false) },
            resid: resmon.create_res(1), 
        }
    }
}

impl SyncRes for MutexSpin {
    fn getid(&self) -> u32 {
        self.resid
    }
}

impl Mutex for MutexSpin {
    /// Lock the spinlock mutex
    fn lock(&self) -> i32 {
        #[cfg(feature = "debug_mutx")]
        trace!("kernel: MutexSpin::lock");
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                self.need();
                if let Some(_) = self.check() {
                    return DEAD_LOCK;
                }
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                self.acquire();
                return 0;
            }
        }
    }

    fn unlock(&self) {
        #[cfg(feature = "debug_mutx")]
        trace!("kernel: MutexSpin::unlock");
        let mut locked = self.locked.exclusive_access();
        *locked = false;
        self.release();
    }
}

/// Blocking Mutex struct
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
    resid: u32,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    /// Create a new blocking mutex
    pub fn new() -> Self {
        #[cfg(feature = "debug_mutx")]
        trace!("kernel: MutexBlocking::new");
        let curproc = current_process();
        let mut resmon = curproc.resmon.exclusive_access();
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
            resid: resmon.create_res(1), 
        }
    }
}

impl SyncRes for MutexBlocking {
    fn getid(&self) -> u32 {
        self.resid
    }
}

impl Mutex for MutexBlocking {
    /// lock the blocking mutex
    fn lock(&self) -> i32 {
        #[cfg(feature = "debug_mutx")]
        trace!("kernel: MutexBlocking::lock");
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            self.need();
            if let Some(_) = self.check() {
                return DEAD_LOCK;
            }
            block_current_and_run_next();
        } else {
            mutex_inner.locked = true;
            self.acquire();
        }
        0
    }

    /// unlock the blocking mutex
    fn unlock(&self) {
        #[cfg(feature = "debug_mutx")]
        trace!("kernel: MutexBlocking::unlock");
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            wakeup_task(waking_task);
        } else {
            mutex_inner.locked = false;
        }
        self.release();
    }
}
