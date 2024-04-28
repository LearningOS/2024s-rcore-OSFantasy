//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next,  get_current_task_status, TaskInfo,
        current_user_token,
        current_task_m_map,current_task_m_unmap,
        add_task_syscall_times,
    },
    mm::{translated_byte_buffer, mm_map, mm_unmap},
    timer::get_time_us,
};


#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
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
pub fn sys_get_time(ts: *const u8, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");

    let token = current_user_token();
    let v = translated_byte_buffer(token, ts, 8);
    let ts_ptr = v[0].as_ptr() as *mut TimeVal;
    let us = get_time_us();
    let time = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };

    unsafe {
        *ts_ptr = time;
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *const u8) -> isize {
    trace!("kernel: sys_task_info");
    let token = current_user_token();
    let v = translated_byte_buffer(token, ti, 8);
    let ti_ptr = v[0].as_ptr() as *mut TaskInfo;
    let task_info = get_current_task_status();

    unsafe {
        *ti_ptr = task_info;
    }
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    // let token = current_user_token();
    // mm_map(token, start, len, port)
    current_task_m_map(start, len, port)

    // -1
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    // let token = current_user_token();
    // mm_unmap(token, start, len)
    current_task_m_unmap(start, len)
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
