//! Process management syscalls
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, current_user_token, get_syscall_count, mmap, munmap, check_permission};
use crate::timer::get_time_us;
use crate::mm::{translated_byte_buffer, MapPermission};
use crate::config::{PAGE_SIZE, TRAP_CONTEXT_BASE, MEMORY_END};
const VA_WIDTH_SV39: usize = 39;


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
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let sec_ptr = ts as *mut usize;
    let usec_ptr = unsafe { sec_ptr.add(1) };

    for buf in translated_byte_buffer(current_user_token(), sec_ptr as *const u8, 8) {
        buf.copy_from_slice(&(us / 1_000_000usize).to_le_bytes());
    }
    
    // 写入微秒字段
    for buf in translated_byte_buffer(current_user_token(), usec_ptr as *const u8, 8) {
        buf.copy_from_slice(&(us % 1_000_000usize).to_le_bytes());
    }
    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        0 => {
            let value;
            if id >= (1 << (VA_WIDTH_SV39 - 1)) {
                if id | (!((1 << VA_WIDTH_SV39) - 1)) >= TRAP_CONTEXT_BASE {
                    return -1;
                }
            } 
            else {
                if id < MEMORY_END && id >= 0x80200000 {
                    return -1;
                }                
            }

            if check_permission(id, MapPermission::R | MapPermission::U) == false {
                return -1;
            }
            let buffers = translated_byte_buffer(current_user_token(), id as *const u8, 1);
            if buffers.is_empty() || buffers[0].len() != 1 {
                return -1;
            }
            value = buffers[0][0];
            value as isize
        }
        1 => {
            if id >= (1 << (VA_WIDTH_SV39 - 1)) {
                if id | (!((1 << VA_WIDTH_SV39) - 1)) >= TRAP_CONTEXT_BASE {
                    return -1;
                }
            } 
            else {
                if id < MEMORY_END && id >= 0x80200000 {
                    return -1;
                }              
            }
            
            if check_permission(id, MapPermission::W | MapPermission::U) == false {
                return -1;
            }

            let mut buffers = translated_byte_buffer(current_user_token(), id as *mut u8, 1);
            if buffers.is_empty() || buffers[0].len() != 1 {
                return -1;
            }
            buffers[0].copy_from_slice(&[data as u8]);
            0
        } 
        2 => {
            get_syscall_count(id)             
        }    
            _ => -1,
    }
      
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap!");
    // 参数检查
    if start % PAGE_SIZE != 0 
        || (prot & !0x7) != 0 
        || (prot & 0x7) == 0 
    {
        return -1;
    }

    // 转换权限标志
    let mut perm = MapPermission::U;
    if (prot & 1) != 0 { perm |= MapPermission::R; }
    if (prot & 2) != 0 { perm |= MapPermission::W; }
    if (prot & 4) != 0 { perm |= MapPermission::X; }

    mmap(start, len, perm)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap!");
    munmap(start, len)
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
