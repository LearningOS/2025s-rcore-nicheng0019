//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, suspend_current_and_run_next},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

use spin::Mutex;
const MAX_ENTRIES: usize = 128;

// 计数器条目结构：存储键值对
#[derive(Clone, Copy)]
struct Entry {
    key: usize,
    count: isize,
}

// 计数器集合结构
struct Counter {
    entries: [Entry; MAX_ENTRIES],
    len: usize,
}


static COUNTER: Mutex<Counter> = Mutex::new(Counter {
    entries: [Entry { key: 0, count: 0 }; MAX_ENTRIES],
    len: 0,
});

// TODO: implement the syscall
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    unsafe {
        match _trace_request {
            0 => *(_id as *const u8) as isize,
            1 => {
                *(_id as *mut u8) = _data as u8;
                0
            } 
            2 => {
                let counter = COUNTER.lock();
                for i in 0..counter.len {
                    if counter.entries[i].key == _id {
                        return counter.entries[i].count;
                    }
                }
                0                
            }
            3 => {
                let mut counter = COUNTER.lock();
                let current_len = counter.len;               
                
                // 查找或插入条目
                for i in 0..counter.len {
                    if counter.entries[i].key == _id {
                        counter.entries[i].count += 1;
                        return counter.entries[i].count;
                    }
                }
              
                if counter.len < MAX_ENTRIES {
                    counter.entries[current_len] = Entry {
                        key: _id,
                        count: 1,
                    };
                    counter.len += 1;
                    1
                } else {
                    0 
                }
                
            }
            _ => -1,
        }
    }  
}
