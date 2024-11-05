use alloc::collections::{ VecDeque, };

use crate::{
    task::{
        current_process, current_task,
    },
};

///
pub const DEAD_LOCK: i32 = -0xDEAD;
type Available = VecDeque<i32>;
type Allocation = VecDeque<VecDeque<u32>>;
type Need = VecDeque<VecDeque<u32>>;

///
pub trait   SyncRes {
    ///
    fn getid(&self) -> u32;

    ///
    fn acquire(&self) {
        let resid = self.getid() as usize;
        let curproc = current_process();
        let mut resmon = curproc.resmon.exclusive_access();
        if resmon.need.len() < current_task().unwrap().get_tid().unwrap() + 1 {
            let mut value = VecDeque::<u32>::new();
            value.resize(resmon.avail.len(), 0);
            resmon.need.resize(current_task().unwrap().get_tid().unwrap() + 1, value);
        }
        if resmon.alloc.len() < current_task().unwrap().get_tid().unwrap() + 1 {
            let mut value = VecDeque::<u32>::new();
            value.resize(resmon.avail.len(), 0);
            resmon.alloc.resize(current_task().unwrap().get_tid().unwrap() + 1, value);
        }
        let reslen = resmon.avail.len();
        if resmon.need[0].len() < reslen {
            for i in 0..resmon.need.len() {
                resmon.need[i].resize(reslen, 0);
            }
        }
        if resmon.alloc[0].len() < reslen {
            for i in 0..resmon.alloc.len() {
                resmon.alloc[i].resize(reslen, 0);
            }
        }
        #[cfg(feature = "debug_syncres")]
        trace!("Pid@{} Acquiring SyncRes: task={}/{} res={:?} res[{}]={}",
                current_task().unwrap().process.upgrade().unwrap().getpid(),
                current_task().unwrap().get_tid().unwrap(),
                resmon.alloc.len(),
                resmon.avail,
                resid, resmon.avail[resid],

            );
        resmon.avail[resid] -= 1;
        resmon.alloc[current_task().unwrap().get_tid().unwrap()][resid] += 1;
        if resmon.need[current_task().unwrap().get_tid().unwrap()][resid] > 0 {
            resmon.need[current_task().unwrap().get_tid().unwrap()][resid] = 0;
        }
    }

    ///
    fn need(&self) {
        let resid = self.getid() as usize;
        let curproc = current_process();
        let mut resmon = curproc.resmon.exclusive_access();
        if resmon.need.len() < current_task().unwrap().get_tid().unwrap() + 1 {
            let mut value = VecDeque::<u32>::new();
            value.resize(resmon.avail.len(), 0);
            resmon.need.resize(current_task().unwrap().get_tid().unwrap() + 1, value);
        }
        if resmon.alloc.len() < current_task().unwrap().get_tid().unwrap() + 1 {
            let mut value = VecDeque::<u32>::new();
            value.resize(resmon.avail.len(), 0);
            resmon.alloc.resize(current_task().unwrap().get_tid().unwrap() + 1, value);
        }
        let reslen = resmon.avail.len();
        if resmon.need[0].len() < reslen {
            for i in 0..resmon.need.len() {
                resmon.need[i].resize(reslen, 0);
            }
        }
        if resmon.alloc[0].len() < reslen {
            for i in 0..resmon.alloc.len() {
                resmon.alloc[i].resize(reslen, 0);
            }
        }
        if resmon.need[current_task().unwrap().get_tid().unwrap()][resid] > 0 {
            return;
        }

        #[cfg(feature = "debug_syncres")]
        trace!("Pid@{} Need SyncRes: task={}/{} res={:?} res[{}]={}",
                current_task().unwrap().process.upgrade().unwrap().getpid(),
                current_task().unwrap().get_tid().unwrap(),
                resmon.alloc.len(),
                resmon.avail,
                resid, resmon.avail[resid],

            );
        resmon.need[current_task().unwrap().get_tid().unwrap()][resid] += 1;
    }
    
    ///
    fn release(&self) {
        let resid = self.getid() as usize;
        let curproc = current_process();
        let mut resmon = curproc.resmon.exclusive_access();
        resmon.avail[resid] += 1;
        resmon.alloc[current_task().unwrap().get_tid().unwrap()][resid] -= 1;
    }

    ///
    fn check(&self) -> Option<u32> {
        let resid = self.getid() as usize;
        let curproc = current_process();
        if !curproc.detect_deadlock() {
            return None;
        }
        let mut resmon = curproc.resmon.exclusive_access();
        if resmon.need.len() < current_task().unwrap().get_tid().unwrap() + 1 {
            let mut value = VecDeque::<u32>::new();
            value.resize(resmon.avail.len(), 0);
            resmon.need.resize(current_task().unwrap().get_tid().unwrap() + 1, value);
        }
        if resmon.alloc.len() < current_task().unwrap().get_tid().unwrap() + 1 {
            let mut value = VecDeque::<u32>::new();
            value.resize(resmon.avail.len(), 0);
            resmon.alloc.resize(current_task().unwrap().get_tid().unwrap() + 1, value);
        }
        let reslen = resmon.avail.len();
        if resmon.need[0].len() < reslen {
            for i in 0..resmon.need.len() {
                resmon.need[i].resize(reslen, 0);
            }
        }
        if resmon.alloc[0].len() < reslen {
            for i in 0..resmon.alloc.len() {
                resmon.alloc[i].resize(reslen, 0);
            }
        }

        let mut finish = [true; 1024];
        for i in 0..resmon.alloc.len() {
            finish[i] = false;
        }
        let mut budget = resmon.avail.clone();
        let mut progress = true;
        #[cfg(feature = "debug_syncres")]
        trace!("Pid@{} Deadlock Checking: task={} res={}",
                current_task().unwrap().process.upgrade().unwrap().getpid(),
                current_task().unwrap().get_tid().unwrap(),
                resid,
            );
        while progress {
            progress = false;

            for t in 0..resmon.alloc.len() {
                if finish[t] {
                    continue;
                } 
                
                let mut fulfil = true;
                for r in 0..budget.len() {
                    #[cfg(feature = "debug_syncres")]
                    trace!("Pid@{} Deadlock Checking: t={} r={} need[{}][{}]={} budget[{}]={}",
                            current_task().unwrap().process.upgrade().unwrap().getpid(),
                            t, r,
                            t, r, resmon.need[t][r],
                            r, budget[r]
                        );
                    if resmon.need[t][r] as i32 > 0 &&
                        resmon.need[t][r] as i32 > budget[r] {
                        fulfil = false;
                        break;
                    }
                }
                if fulfil {
                    #[cfg(feature = "debug_syncres")]
                    trace!("Pid@{} Deadlock Checking: task={} inc budget",
                        current_task().unwrap().process.upgrade().unwrap().getpid(),
                        current_task().unwrap().get_tid().unwrap(),              
                    );
                    progress = true;
                    for r in 0..budget.len() {
                        budget[r] += resmon.alloc[t][r] as i32;
                        finish[t] = true;
                    }
                    #[cfg(feature = "debug_syncres")]
                    trace!("Pid@{} Deadlock Checking: budget={:?}",
                        current_task().unwrap().process.upgrade().unwrap().getpid(),
                        budget,
                    );
                }
            }
        }

        for (i, f) in finish.iter().enumerate() {
            if !f {
                return Some(i as u32);
            }
        }
        None
    }
}

///
pub struct  ResMonitor {
    avail: Available,
    alloc: Allocation,
    need: Need,
}

impl ResMonitor {
    ///
    pub fn new() -> Self {
        Self {
            avail: VecDeque::<i32>::new(),
            alloc: VecDeque::<VecDeque::<u32>>::new(),
            need: VecDeque::<VecDeque::<u32>>::new(),
        }
    }

    ///
    pub fn create_res(&mut self, num: u32) -> u32 {
        self.avail.push_back(num as i32);
        (self.avail.len() - 1) as u32
    }

    ///
    pub fn dump_res(&self) {
        trace!("AVAIL: {:?}", self.avail);
        trace!("ALLOC: {:?}", self.alloc);
        trace!("NEED: {:?}", self.need);
    }
}