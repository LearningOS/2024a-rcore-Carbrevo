//! Process management syscalls
//!
use alloc::sync::Arc;

use crate::{
    config::{MAX_SYSCALL_NUM, BIG_STRIDE},
    fs::{open_file, OpenFlags},
    mm::{translated_refmut, translated_str},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskStatus,
    },
    mm::{*},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
///
pub struct TimeVal {
    ///
    pub sec: usize,
    ///
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
pub fn sys_exit(exit_code: i32) -> ! {
    #[cfg(feature="debug_exit")]
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    #[cfg(feature="debug_exit")]
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
    suspend_current_and_run_next();
    0
}

///
pub fn sys_getpid() -> isize {
    #[cfg(feature="debug_exit")]
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

///
pub fn sys_fork() -> isize {
    #[cfg(feature="debug_exit")]
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

///
pub fn sys_exec(path: *const u8) -> isize {
    #[cfg(feature="debug_exit")]
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    #[cfg(feature="debug_exit")]
    trace!("kernel::pid[{}] sys_waitpid [{}]", current_task().unwrap().pid.0, pid);
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    #[cfg(feature="debug_exit")]
    trace!(
        "kernel:pid[{}] sys_get_time",
        current_task().unwrap().pid.0
    );

    let curtsk = current_task().unwrap();
    let task_inner = curtsk.inner_exclusive_access();
    let virt_ts = VirtAddr::from(_ts as usize);
    let pge_ts = task_inner.memory_set.translate(virt_ts.floor()).unwrap();
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
    #[cfg(feature="debug_exit")]
    trace!(
        "kernel:pid[{}] sys_task_info",
        current_task().unwrap().pid.0
    );

    let curtsk = current_task().unwrap();
    let task_inner = curtsk.inner_exclusive_access();
    let virt_ts = VirtAddr::from(_ti as usize);
    let pge_ts = task_inner.memory_set.translate(virt_ts.floor()).unwrap();
    let ti = PhysAddr::from(usize::from(PhysAddr::from(pge_ts.ppn())) + virt_ts.page_offset()).get_mut::<TaskInfo>();
    drop(task_inner);

    *ti = curtsk.get_taskinfo();
    0
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    #[cfg(feature="debug_exit")]
    trace!(
        "kernel:pid[{}] sys_mmap",
        current_task().unwrap().pid.0
    );

    let curtsk = current_task().unwrap();
    let mut task_inner = curtsk.inner_exclusive_access();

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
    #[cfg(feature="debug_exit")]
    trace!("Checking map: [{:?}, {:?})", pgn_start, pgn_end);
    if (usize::from(pgn_start)..usize::from(pgn_end))
                .into_iter()
                .map(|pg|task_inner.memory_set.translate(VirtPageNum::from(pg)))
                .any(|x|if let Some(pte) = x { if pte.is_valid() { trace!("Found mmapped {:?}", pte); true} else {false}  } else {false}) {
            warn!("kernel: mmap part of range mapped!");
        return -1;                    
    }

    task_inner.memory_set.insert_framed_area(virt_start, virt_end, MapPermission::from(_port));
    0
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    #[cfg(feature="debug_exit")]
    trace!(
        "kernel:pid[{}] sys_munmap",
        current_task().unwrap().pid.0
    );
    let curtsk = current_task().unwrap();
    let mut task_inner = curtsk.inner_exclusive_access();

    let virt_start = VirtAddr::from(_start);

    if !virt_start.aligned() {
        warn!("kernel: mmap start is not aligned!");
        return -1;
    }

    let virt_end = VirtAddr::from(_start + _len);
    let pgn_start = virt_start.floor();
    let pgn_end = virt_end.ceil();
    #[cfg(feature="debug_exit")]
    trace!("Checking map: [{:?}, {:?})", pgn_start, pgn_end);
    if !(usize::from(pgn_start)..usize::from(pgn_end))
            .into_iter().map(|pg|task_inner.memory_set.translate(VirtPageNum::from(pg)))
            .all(|x|if let Some(pte) = x { if pte.is_valid() { trace!("Found mmapped {:?}", pte); true} else {false}  } else {false}) {
        warn!("kernel: munmap part of range not mapped!");
        return -1;                    
    }

    task_inner.memory_set.remove_area_with_start_vpn(pgn_start);
    0
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    #[cfg(feature="debug_exit")]
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    #[cfg(feature="debug_exit")]
    trace!(
        "kernel:pid[{}] sys_spawn",
        current_task().unwrap().pid.0
    );

    let token = current_user_token();
    let path = translated_str(token, _path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let current_task = current_task().unwrap();
        let new_task = current_task.spawn();
        let new_pid = new_task.pid.0;
        // modify trap context of new_task, because it returns immediately after switching
        let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
        // we do not have to move to next instruction since we have done it before
        // for child process, fork returns 0
        trap_cx.x[10] = 0;

        //let task = current_task().unwrap();
        new_task.exec(all_data.as_slice());
        #[cfg(feature="debug_exit")]
        trace!("kernel:pid[{}] exec '{}' on spawn", new_pid, path);

        // add new task to scheduler
        add_task(new_task);
        new_pid as isize        
    } else {
        warn!("kernel:pid[{}] spawn failed: Invalid file name", path);
        -1
    }
}

// YOUR JOB: Set task priority.
///
pub fn sys_set_priority(_prio: isize) -> isize {
    #[cfg(feature="debug_exit")]
    trace!(
        "kernel:pid[{}] sys_set_priority",
        current_task().unwrap().pid.0
    );

    if _prio < 2 {
        warn!("kernel:pid[{}] set_priority failed: Invalid priority", _prio);
        return -1;
    }

    let curtsk = current_task().unwrap();
    let mut task_inner = curtsk.inner_exclusive_access();

    //task_inner.stride = 0;
    task_inner.pass = _prio as usize / BIG_STRIDE;
    _prio
}
