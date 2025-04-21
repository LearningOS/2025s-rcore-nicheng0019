//! File and filesystem-related syscalls
use crate::fs::{open_file, OpenFlags, Stat, add_file_link, remove_file_link, OSInode, get_link_num, get_file_name};
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer, translated_refmut};
use crate::task::{current_task, current_user_token};
use alloc::sync::Arc;
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    info!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
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
    info!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    info!("sys_read {} {}", fd, inner.fd_table.len());
    if fd >= inner.fd_table.len() {
        return -1;
    }
    //info!("kernel:pid[{}] sys_read {}", current_task().unwrap().pid.0, len);
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap(), true) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        info!("kernel:pid[{}] sys_open fd={}", current_task().unwrap().pid.0, fd);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    info!("kernel:pid[{}] sys_close {}", current_task().unwrap().pid.0, fd);
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
pub fn sys_fstat(fd: usize, st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat",
        current_task().unwrap().pid.0
    );
    let task = current_task().unwrap();
    let token = current_user_token();
    let inner = task.inner_exclusive_access();
    
    // 检查文件描述符有效性
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = unsafe {
            &*(Arc::as_ptr(file) as *const OSInode)
        };

        // 转换用户空间指针
        let st = translated_refmut(token, st);
        st.dev = 0; // 按需求固定为0
        st.ino = file.get_inode_id() as u64;
        st.nlink = 1; // 硬链接数初始为1

        if let Some(name) = get_file_name(file.get_inode_id()) 
        {
            st.nlink = st.nlink + get_link_num(name.as_str());
        }
        
        // 判断文件类型
        st.mode = file.get_file_type();
        
        0
    } else {
        -1
    }
}

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(oldpath: *const u8, newpath: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let old_path = translated_str(token, oldpath);
    let new_path = translated_str(token, newpath);

    if old_path == new_path {
        return -1;
    }

    // 获取原文件 inode
    let _old_inode = match open_file(&old_path, OpenFlags::RDONLY, true) {
        Some(inode) => inode,
        None => return -1,
    };

    add_file_link(old_path.as_str(), new_path.as_str());
    0
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(name: *const u8) -> isize {
    info!(
        "kernel:pid[{}] sys_unlinkat {:#?}",
        current_task().unwrap().pid.0, name
    );
    let token = current_user_token();
    let name = translated_str(token, name);

    // 获取原文件 inode
    let _old_inode = match open_file(&name, OpenFlags::RDONLY, true) {
        Some(inode) => inode,
        None => return -1,
    };

    remove_file_link(name.as_str())
}
