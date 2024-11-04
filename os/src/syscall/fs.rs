//! File and filesystem-related syscalls
#![allow(unused_imports)]
use core::ffi::{ CStr, c_char };

use crate::fs::{ROOT_INODE, open_file, OpenFlags, Stat, StatMode};
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer};
use crate::task::{current_task, current_user_token};
use easy_fs::StatMode as VfsStatMode;
use crate::mm::{*};

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    #[cfg(feature="debug_exit")]
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    #[cfg(feature="debug_exit")]
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        #[cfg(feature="debug_exit")]
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    #[cfg(feature="debug_open")]
    trace!("kernel:pid[{}] sys_open", 
            current_task().unwrap().pid.0,
        );
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        #[cfg(feature="debug_open")]
        trace!("kernel:pid[{}] sys_open: file={} fd={}", 
                current_task().unwrap().pid.0,
                path, fd
            );
            fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    #[cfg(feature="debug_close")]
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(_fd: usize, _st: *mut Stat) -> isize {
    #[cfg(feature="debug_fstat")]
    trace!(
        "kernel:pid[{}] sys_fstat: fd={}",
        current_task().unwrap().pid.0,
        _fd,
    );
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if let Some(file) = &inner.fd_table[_fd] {
        let inode = file.inode().unwrap();
        let ino = inode.inode_id();
        let mode: StatMode = unsafe { let m = inode.mode(); *(&m as *const VfsStatMode as *const StatMode)};
        let nlink = ROOT_INODE.find_by_id(ino).len();

        #[cfg(feature="debug_fstat")]
        trace!("kernel:pid[{}] sys_fstat@{}: ino={}, mode={:?}, nlink={}",
                current_task().unwrap().pid.0,
            _fd, ino, mode, nlink);
    
        let virt_st = VirtAddr::from(_st as usize);
        let pge_st = inner.memory_set.translate(virt_st.floor()).unwrap();
        let st = PhysAddr::from(usize::from(PhysAddr::from(pge_st.ppn())) + virt_st.page_offset()).get_mut::<Stat>();
        *st = Stat {
            dev: 0,
            ino: ino as u64,
            mode,
            nlink: nlink as u32,
            pad: [0u64;7],
        };
        0    
    } else {
        -1
    }
}

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    #[cfg(feature="debug_link")]
    trace!(
        "kernel:pid[{}] sys_linkat",
        current_task().unwrap().pid.0
    );
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let virt_oldname = VirtAddr::from(_old_name as usize);
    let pge_oldname = inner.memory_set.translate(virt_oldname.floor()).unwrap();
    let kaddr_oldname = PhysAddr::from(usize::from(PhysAddr::from(pge_oldname.ppn())) + virt_oldname.page_offset());
    let virt_newname = VirtAddr::from(_new_name as usize);
    let pge_newname = inner.memory_set.translate(virt_newname.floor()).unwrap();
    let kaddr_newname = PhysAddr::from(usize::from(PhysAddr::from(pge_newname.ppn())) + virt_newname.page_offset());

    let old_name: &str = unsafe { CStr::from_ptr(usize::from(kaddr_oldname) as *const c_char).to_str().unwrap() };
    let new_name: &str = unsafe { CStr::from_ptr(usize::from(kaddr_newname) as *const c_char).to_str().unwrap() };

    if old_name == new_name {
        warn!("kernel:pid[{}] sys_linkat failed: linkat itself",
                current_task().unwrap().pid.0);
        return -1;
    }
    ROOT_INODE.vfs_link(old_name, new_name)
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(_name: *const u8) -> isize {
    #[cfg(feature="debug_link")]
    trace!(
        "kernel:pid[{}] sys_unlinkat",
        current_task().unwrap().pid.0
    );
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    let virt_name = VirtAddr::from(_name as usize);
    let pge_name = inner.memory_set.translate(virt_name.floor()).unwrap();
    let kaddr_name = PhysAddr::from(usize::from(PhysAddr::from(pge_name.ppn())) + virt_name.page_offset());
    let name: &str = unsafe { CStr::from_ptr(usize::from(kaddr_name) as *const c_char).to_str().unwrap() };

    ROOT_INODE.vfs_unlink(name)
}
