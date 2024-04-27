//! Types related to task management

use crate::config::MAX_SYSCALL_NUM;
use super::TaskContext;

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    pub task_info: TaskInfo,
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
    /// The task Start time(ms)
    pub task_start_time: usize
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

/// Task information
#[allow(dead_code)]
#[derive(Copy, Clone)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

impl TaskInfo {
    pub fn new(status: TaskStatus) -> Self {
        // Initialize syscall_times with zeros.
        let syscall_times = [0; MAX_SYSCALL_NUM];
        // Assuming TaskStatus::Running is a reasonable default.
        TaskInfo {
            status,
            syscall_times,
            time: 0,
        }
    }

    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
    }

    pub fn add_syscall_time(&mut self, index: usize) {
        if index < MAX_SYSCALL_NUM {
            self.syscall_times[index] += 1;
        }
    }

    pub fn increment_time(&mut self, increment: usize) {
        self.time = increment;
    }
}

