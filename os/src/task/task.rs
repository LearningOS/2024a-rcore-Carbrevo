//! Types related to task management

use super::TaskContext;
use crate::config::*;

///
#[derive(Copy, Clone)]
pub struct TaskStatis {
    /// The numbers of syscall called by task
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    pub starttime: usize,
}

impl Default for TaskStatis {
    fn default() -> Self {
        Self {
            syscall_times: [0u32; MAX_SYSCALL_NUM],
            starttime: 0usize,
        }
    }
}

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,

    /// The task statis
    pub statis: TaskStatis,

}

/// The status of a task
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}
