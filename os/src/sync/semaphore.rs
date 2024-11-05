//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use crate::task::{current_process};
use alloc::{collections::VecDeque, sync::Arc};
use crate::sync::{ SyncRes, DEAD_LOCK };

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
    resid: u32,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(res_count: usize) -> Self {
        #[cfg(feature = "debug_sem")]
        trace!("kernel: Semaphore::new");
        let curproc = current_process();
        let mut resmon = curproc.resmon.exclusive_access();
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
            resid: resmon.create_res(res_count as u32), 
        }
    }

    /// up operation of semaphore
    pub fn up(&self) {
        #[cfg(feature = "debug_sem")]
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        self.acquire();
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore
    pub fn down(&self) -> i32 {
        #[cfg(feature = "debug_sem")]
        trace!("kernel: Semaphore::down");
        #[cfg(feature = "debug_sem")]
        {
            let curproc = current_process();
            let resmon = curproc.resmon.exclusive_access();
            resmon.dump_res();
            drop(resmon);    
        }

        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            self.need();
            if let Some(_) = self.check() {
                inner.count += 1;
                return DEAD_LOCK;
            }
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        } else {
            self.acquire();
        }
        0
    }
}

impl SyncRes for Semaphore {
    fn getid(&self) -> u32 {
        self.resid
    }
}

