//! `Arc<Inode>` -> `OSInodeInner`: In order to open files concurrently
//! we need to wrap `Inode` into `Arc`,but `Mutex` in `Inode` prevents
//! file systems from being accessed simultaneously
//!
//! `UPSafeCell<OSInodeInner>` -> `OSInode`: for static `ROOT_INODE`,we
//! need to wrap `OSInodeInner` into `UPSafeCell`
use super::File;
use crate::drivers::BLOCK_DEVICE;
use crate::mm::UserBuffer;
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::*;
use easy_fs::{EasyFileSystem, Inode};
use lazy_static::*;
use alloc::string::String;
use super::StatMode;

/// inode in memory
/// A wrapper around a filesystem inode
/// to implement File trait atop
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPSafeCell<OSInodeInner>,
}
/// The OS inode inner in 'UPSafeCell'
pub struct OSInodeInner {
    offset: usize,
    inode: Arc<Inode>,
}

impl OSInode {
    /// create a new inode in memory
    pub fn new(readable: bool, writable: bool, inode: Arc<Inode>) -> Self {
        Self {
            readable,
            writable,
            inner: unsafe { UPSafeCell::new(OSInodeInner { offset: 0, inode }) },
        }
    }
    /// read all data from the inode
    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer: Vec<u8> = Vec::with_capacity(512);
        buffer.resize(512, 0);
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buffer);
            if len == 0 {
                break;
            }
            inner.offset += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }

    ///get_inode_id
    pub fn get_inode_id(&self) -> u32 {
        self.inner.exclusive_access().inode.get_inode_id()
    }

    /// get_file_type
    pub fn get_file_type(&self) -> StatMode {
        if self.inner.exclusive_access().inode.read_disk_inode(|di| di.is_dir()) {
            StatMode::DIR
        } else {
            StatMode::FILE
        }
    }


    
}

lazy_static! {
    pub static ref ROOT_INODE: Arc<Inode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

/// List all apps in the root directory
pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("**************/");
}

bitflags! {
    ///  The flags argument to the open() system call is constructed by ORing together zero or more of the following values:
    pub struct OpenFlags: u32 {
        /// readyonly
        const RDONLY = 0;
        /// writeonly
        const WRONLY = 1 << 0;
        /// read and write
        const RDWR = 1 << 1;
        /// create new file
        const CREATE = 1 << 9;
        /// truncate file size to 0
        const TRUNC = 1 << 10;
    }
}

impl OpenFlags {
    /// Do not check validity for simplicity
    /// Return (readable, writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

#[derive(Clone)]
/// A file link in the root directory
pub struct FileLink {
    /// The file name
    pub name: String,
    /// link name
    pub link_name: String,
}

lazy_static! {
    /// The file links in the root directory
    pub static ref FILE_LINKS: UPSafeCell<Vec<FileLink>> = {
        unsafe { UPSafeCell::new(Vec::new()) }
    };

    pub static ref REMOVE_FILES: UPSafeCell<Vec<String>> = {
        unsafe { UPSafeCell::new(Vec::new()) }
    };
}

/// Add a file link to the root directory   
pub fn add_file_link(name: &str, link_name: &str) {
    FILE_LINKS.exclusive_access().push(FileLink {
        name: String::from(name),
        link_name: String::from(link_name),
    });
}

/// Add a file link to the root directory   
pub fn get_link_num(name: &str) -> u32 {
    let mut num = FILE_LINKS
        .exclusive_access()
        .iter()
        .filter(|l| l.name == name)
        .count() as u32;
    if REMOVE_FILES.exclusive_access().iter().find(|f| **f == String::from(name)).is_some() {
        num -= 1;
    }
    num
}

/// get_file_type
pub fn get_file_name(inode_id: u32) -> Option<String> {
    ROOT_INODE.get_name_by_id(inode_id) 
}

/// Remove a file from the root directory
fn remove_file(name: &str) -> isize
{
    info!("remove_file");
    if ROOT_INODE.remove(name)
    {
        REMOVE_FILES.exclusive_access().push(String::from(name));
        info!("remove_file finish");
        return 0;
    }
    -1
}

/// Remove a file link from the root directory
pub fn remove_file_link(link_name: &str) -> isize {
    let mut file_links = FILE_LINKS.exclusive_access();
    if let Some(pos) = file_links.iter().position(|link| link.link_name == link_name) {
        file_links.remove(pos);
        return 0;
    }
    if let Some(_pos) = file_links.iter().position(|link| link.name == link_name) {
        let mut remove_files = REMOVE_FILES.exclusive_access();
        remove_files.push(String::from(link_name));
        return 0;
    }
    
    remove_file(link_name)
}

/// Open a file
pub fn open_file(name: &str, flags: OpenFlags, _bfindlink: bool) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();
    info!("open_file {}", name);
    let mut name = name;
    
    let binding = FILE_LINKS.exclusive_access();
    if let Some(link) = binding.iter().find(|l| l.link_name == name) {
        name = &link.name;
    }
    
    info!("open_file 2");
    if flags.contains(OpenFlags::CREATE) {
        let mut remove_files = REMOVE_FILES.exclusive_access();
        if let Some(pos) = remove_files.iter().position(|f| *f == String::from(name)) {
            remove_files.remove(pos);
        }

        info!("open_file 3");
        if let Some(inode) = ROOT_INODE.find(name) {
            // clear size
            inode.clear();
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        } else {
            // create file
            ROOT_INODE
                .create(name)
                .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
        }
    } else {
        info!("open_file 4 {}", name);
        ROOT_INODE.find(name).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            info!("open_file find {}", name);
            Arc::new(OSInode::new(readable, writable, inode))
        })
    }
}

impl File for OSInode {
    fn readable(&self) -> bool {
        self.readable
    }
    fn writable(&self) -> bool {
        self.writable
    }
    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        
        for slice in buf.buffers.iter_mut() {
            let read_size = inner.inode.read_at(inner.offset, *slice);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }
        info!("read total_read_size {}", total_read_size);
        total_read_size
    }
    fn write(&self, buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.offset, *slice);
            assert_eq!(write_size, slice.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
}
