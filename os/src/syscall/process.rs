//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus,
        TASK_MANAGER,
    },
    mm::{*},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    pub status: TaskStatus,
    /// The numbers of syscall called by task
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    pub time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");

    let mut inner = TASK_MANAGER.task_inner();
    let current = inner.current_task;
    let curtsk = &mut inner.tasks[current];
    let virt_ts = VirtAddr::from(_ts as usize);
    let pge_ts = curtsk.memory_set.translate(virt_ts.floor()).unwrap();
    let ts = PhysAddr::from(usize::from(PhysAddr::from(pge_ts.ppn())) + virt_ts.page_offset()).get_mut::<TimeVal>();

    let us = get_time_us();
    *ts = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");

    let mut inner = TASK_MANAGER.task_inner();
    let current = inner.current_task;
    let curtsk = &mut inner.tasks[current];
    let virt_ts = VirtAddr::from(_ti as usize);
    let pge_ts = curtsk.memory_set.translate(virt_ts.floor()).unwrap();
    let ti = PhysAddr::from(usize::from(PhysAddr::from(pge_ts.ppn())) + virt_ts.page_offset()).get_mut::<TaskInfo>();
    drop(inner);

    *ti = TASK_MANAGER.get_taskinfo();
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap");

    let mut inner = TASK_MANAGER.task_inner();
    let current = inner.current_task;
    let curtsk = &mut inner.tasks[current];
    let virt_start = VirtAddr::from(_start);

    if !virt_start.aligned() {
        warn!("kernel: mmap start is not aligned!");
        return -1;
    }

    if ((_port & 0x07) == 0) || ((_port & !0x07) != 0) {
        warn!("kernel: mmap invalid port attr!");
        return -1;
    }

    let virt_end = VirtAddr::from(_start + _len);
    let pgn_start = virt_start.floor();
    let pgn_end = virt_end.ceil();
    trace!("Checking map: [{:?}, {:?})", pgn_start, pgn_end);
    if (usize::from(pgn_start)..usize::from(pgn_end))
                .into_iter()
                .map(|pg|curtsk.memory_set.translate(VirtPageNum::from(pg)))
                .any(|x|if let Some(pte) = x { if pte.is_valid() { trace!("Found mmapped {:?}", pte); true} else {false}  } else {false}) {
            warn!("kernel: mmap part of range mapped!");
        return -1;                    
    }

    curtsk.memory_set.insert_framed_area(virt_start, virt_end, MapPermission::from(_port));
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap");

    let mut inner = TASK_MANAGER.task_inner();
    let current = inner.current_task;
    let curtsk = &mut inner.tasks[current];
    let virt_start = VirtAddr::from(_start);

    if !virt_start.aligned() {
        warn!("kernel: mmap start is not aligned!");
        return -1;
    }

    let virt_end = VirtAddr::from(_start + _len);
    let pgn_start = virt_start.floor();
    let pgn_end = virt_end.ceil();
    trace!("Checking map: [{:?}, {:?})", pgn_start, pgn_end);
    if !(usize::from(pgn_start)..usize::from(pgn_end))
            .into_iter().map(|pg|curtsk.memory_set.translate(VirtPageNum::from(pg)))
            .all(|x|if let Some(pte) = x { if pte.is_valid() { trace!("Found mmapped {:?}", pte); true} else {false}  } else {false}) {
        warn!("kernel: munmap part of range not mapped!");
        return -1;                    
    }

    curtsk.memory_set.remove_framed_area(virt_start, virt_end);
    0
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
