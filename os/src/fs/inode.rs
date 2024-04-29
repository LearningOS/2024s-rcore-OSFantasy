//! `Arc<Inode>` -> `OSInodeInner`: In order to open files concurrently
//! we need to wrap `Inode` into `Arc`,but `Mutex` in `Inode` prevents
//! file systems from being accessed simultaneously
//!
//! `UPSafeCell<OSInodeInner>` -> `OSInode`: for static `ROOT_INODE`,we
//! need to wrap `OSInodeInner` into `UPSafeCell`
use alloc::collections::VecDeque;
use alloc::string::String;
use super::{File, Stat, StatMode};
use crate::drivers::BLOCK_DEVICE;
use crate::mm::UserBuffer;
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::RefMut;
use bitflags::*;
use easy_fs::{EasyFileSystem, Inode};
use lazy_static::*;

pub struct OSInodeManager {
    inode_queue: VecDeque<Arc<OSInode>>,
}

/// inode in memory
/// A wrapper around a filesystem inode
/// to implement File trait atop
pub struct OSInode {
    readable: bool,
    writable: bool,
    stat: Stat,
    name: String,
    inner: UPSafeCell<OSInodeInner>,
}
/// The OS inode inner in 'UPSafeCell'
pub struct OSInodeInner {
    offset: usize,
    inode: Arc<Inode>,
}

pub struct LinkName {
    old_path: String,
    new_path: String,
}

pub struct LinkManager {
    name_queue: VecDeque<Arc<LinkName>>,
}

impl OSInode {
    /// create a new inode in memory
    pub fn new(readable: bool, writable: bool, inode: Arc<Inode>, ino: u64, nlink: u32,stat_mode: StatMode, name: String) -> Self {
        Self {
            readable,
            writable,
            inner: unsafe { UPSafeCell::new(OSInodeInner { offset: 0, inode }) },
            stat: Stat::new(ino, nlink, stat_mode),
            name,
        }
    }
    /// read all data from the inode
    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];
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
        LINK_MANAGER.exclusive_access().add(app.clone().as_str(), "none_name_just_test_made_by_OSFantasy");
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



/// Open a file
pub fn open_file(name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();

    let mut link_manager = LINK_MANAGER.exclusive_access();
    let (name, nlink, index)= link_manager.all(name, flags.clone());
    if flags.contains(OpenFlags::CREATE) {
        if let Some(inode) = ROOT_INODE.find(name) {
            // clear size
            inode.clear();
            Some(Arc::new(OSInode::new(readable, writable, inode, index as u64, nlink as u32, StatMode::FILE, String::from(name))))
        } else {
            // create file
            ROOT_INODE
                .create(name)
                .map(|inode| Arc::new(OSInode::new(readable, writable, inode, index as u64, nlink as u32, StatMode::FILE, String::from(name))))
        }
    } else {
        if nlink != 0 {
        ROOT_INODE.find(name).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            Arc::new(OSInode::new(readable, writable, inode, index as u64, nlink as u32, StatMode::FILE, String::from(name)))
        })
        } else {
            None
        }
    }
}

// pub fn update_file(name: &str, flags: OpenFlags){
//
// }

// impl OSInode {
//     pub fn exclusive_access(&self) -> RefMut<'_, OSInode> {
//         self.exclusive_access()
//     }
// }

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

    fn file_stat(&self) -> Stat {
        let mut stat = self.stat.clone();
        let name = self.name.as_str();
        let mut link_manager = LINK_MANAGER.exclusive_access();
        let (name, nlink, index)= link_manager.all(name, OpenFlags::RDWR);
        stat.nlink = nlink as u32;
        stat.ino = index as u64;
        stat
    }
}

impl LinkManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            name_queue: VecDeque::new(),
        }
    }

    pub fn all<'a>(&'a mut self, name: &'a str, flags: OpenFlags) -> (&'a str, usize, usize) {
        if flags.contains(OpenFlags::CREATE) {
            println!("[Kernel][link]all , add:{}", name.clone());
            self.add(name.clone(), "none_name_just_test_made_by_OSFantasy");
        }
        let fetched_name = self.fetch(name);
        let nlink = self.find_num(&fetched_name);
        let index = self.find_index(&fetched_name);
        (fetched_name, nlink, index)
    }

    /// Add process back to ready queue
    pub fn add(&mut self, old_name: &str, new_name: &str) -> isize {
        if old_name == new_name {
            return -1;
        }

        let link_name = LinkName {
            old_path: old_name.parse().unwrap(),
            new_path: new_name.parse().unwrap(),
        };
        self.name_queue.push_back(Arc::from(link_name));
        0
    }

    pub fn remove(&mut self, name: &str) -> isize {
        let mut result: isize = -1;
        let mut remove_index: usize = 0;

        for (index, link_name) in self.name_queue.iter().enumerate() {
            let old_name = link_name.old_path.as_str();
            let new_name = link_name.new_path.as_str();
            if old_name == name || new_name == name  {
                remove_index = index;
                result = 0;
                println!("find remove_index is {}, old_name = {}, new_name = {}", remove_index, old_name, new_name);
                break;
            }
        }

        if result == 0 {
            self.name_queue.remove(remove_index);
        }

        result
    }
    /// Take a process out of the ready queue
    pub fn fetch<'a>(&'a self, name: &'a str) -> &'a str {
        if let Some(index) = self.name_queue.iter().position(|link_name| {
            Arc::clone(link_name).old_path == name || Arc::clone(link_name).new_path == name
        }) {
            self.name_queue[index].old_path.as_str()
        } else {
            println!("[Kernel][fs][inode]Not fetch the name in LINK_MANAGER");
            name
        }
    }

    pub fn find_num(&self, name: &str) -> usize {
        let count = self.name_queue.iter().filter(|link_name| {
            Arc::clone(link_name).old_path == name
        }).count();

        if count == 0 {
            println!("[Kernel][fs][inode] Not fetch the name in LINK_MANAGER");
        }

        count
    }

    pub fn find_index(&self, name: &str) -> usize {
        if let Some(index) = self.name_queue.iter().position(|link_name| {
            Arc::clone(link_name).old_path == name
        }) {
            return index;
        } else {
            self.name_queue.len()
        }
    }

}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref LINK_MANAGER: UPSafeCell<LinkManager> =
        unsafe { UPSafeCell::new(LinkManager::new()) };
}