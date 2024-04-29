//! File trait & inode(dir, file, pipe, stdin, stdout)

mod inode;
mod stdio;

use crate::mm::UserBuffer;

/// trait File for all file types
pub trait File: Send + Sync {
    /// the file readable?
    fn readable(&self) -> bool;
    /// the file writable?
    fn writable(&self) -> bool;
    /// read from the file to buf, return the number of bytes read
    fn read(&self, buf: UserBuffer) -> usize;
    /// write to the file from buf, return the number of bytes written
    fn write(&self, buf: UserBuffer) -> usize;

    fn file_stat(&self) -> Stat;
}

/// The stat of a inode
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Stat {
    /// ID of device containing file
    pub dev: u64,
    /// inode number
    pub ino: u64,
    /// file type and mode
    pub mode: StatMode,
    /// number of hard links
    pub nlink: u32,
    /// unused pad
    pad: [u64; 7],
}

bitflags! {
    /// The mode of a inode
    /// whether a directory or a file
    pub struct StatMode: u32 {
        /// null
        const NULL  = 0;
        /// directory
        const DIR   = 0o040000;
        /// ordinary regular file
        const FILE  = 0o100000;
    }
}

impl Stat {
    pub fn new(ino: u64, nlink: u32,mode: StatMode) -> Self {
        // 确保结构体的对齐方式与 C 语言兼容
        let pad: [u64; 7] = [0;7];
        Stat {
            dev: 0,
            ino,
            mode,
            nlink,
            pad,
        }
    }
}

pub use inode::{list_apps, open_file, OSInode, OpenFlags, LINK_MANAGER};
pub use stdio::{Stdin, Stdout};
