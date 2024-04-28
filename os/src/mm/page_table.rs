//! Implementation of [`PageTableEntry`] and [`PageTable`].

use super::{frame_alloc, FrameTracker, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
use alloc::vec;
use alloc::vec::Vec;
use alloc::string::String;
use core::fmt::Debug;
use bitflags::*;

bitflags! {
    /// page table entry flags
    pub struct MmmapPort: usize {
        const R = 1 << 0;
        const W = 1 << 1;
        const X = 1 << 2;
    }
}

bitflags! {
    /// page table entry flags
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

impl PTEFlags {
    /// Tries to convert a `usize` to `PTEFlags`, truncating if necessary.
    pub fn from_usize(value: usize) -> Self {
        // Truncate the `usize` to `u8` by casting
        let truncated_value = value as u8;
        // SAFETY: We assume that the truncated value is a valid set of flags
        unsafe { Self::from_bits_unchecked(truncated_value) }
    }
}


#[derive(Copy, Clone)]
#[repr(C)]
/// page table entry structure
pub struct PageTableEntry {
    /// bits of page table entry
    pub bits: usize,
}

impl PageTableEntry {
    /// Create a new page table entry
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }
    /// Create an empty page table entry
    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }
    /// Get the physical page number from the page table entry
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }
    /// Get the flags from the page table entry
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
    /// The page pointered by page table entry is valid?
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    /// The page pointered by page table entry is readable?
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    /// The page pointered by page table entry is writable?
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    /// The page pointered by page table entry is executable?
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

/// page table structure
pub struct PageTable {
    root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

/// Assume that it won't oom when creating/mapping.
impl PageTable {
    /// Create a new page table
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }
    /// Temporarily used to get arguments from user space.
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }
    /// Find PageTableEntry by VirtPageNum, create a frame for a 4KB page table if not exist
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }
    /// Find PageTableEntry by VirtPageNum
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }
    /// set the map between virtual page number and physical page number
    #[allow(unused)]
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) -> isize{
        let pte = self.find_pte_create(vpn).unwrap();
        // assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        if pte.is_valid() {
            println!("vpn is mapped before mapping");
            return -1
        }
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
        0
    }
    /// remove the map between virtual page number and physical page number
    #[allow(unused)]
    pub fn unmap(&mut self, vpn: VirtPageNum) -> isize {
        let pte = self.find_pte(vpn).unwrap();
        // assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        if !pte.is_valid() {
            println!("vpn is invalid before unmapping");
            return -1
        }
        *pte = PageTableEntry::empty();
        0
    }
    /// get the page table entry from the virtual page number
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }
    /// get the token from the page table
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

/// Translate&Copy a ptr[u8] array with LENGTH len to a mutable u8 Vec through page table
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    // println!("[Kernel][translated_byte_buffer]start = {}, len = {}", start, len);
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));

        // println!("[Kernel][translated_byte_buffer]start_va = {}", usize::from(start_va));
        // println!("[Kernel][translated_byte_buffer]vpn = {}", usize::from(vpn));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}

fn allocate_free_ppn() -> Option<PhysPageNum> {
    // 调用frame_alloc函数来分配一个空闲的物理页帧
    frame_alloc().map(|frame| frame.ppn)
}

pub fn mm_map(token: usize, start: usize, len: usize, port: usize) -> isize{
    let flags = PTEFlags::from_usize((port << 1) | 0x09);
    let mut page_table = PageTable::from_token(token);
    let mut result = 0;
    println!("[Kernel][mm_map]start = {}, len = {}", start, len);
    println!("[Kernel][mm_map]flags = {:?}", flags);
    let mut sta = start;
    let end = start + len;
    while sta < end {
        let start_va = VirtAddr::from(sta);
        let mut vpn = start_va.floor();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));

        println!("[Kernel][mm_map]start_va = {}", usize::from(start_va));
        println!("[Kernel][mm_map]vpn = {}", usize::from(vpn));

        if let Some(ppn) = allocate_free_ppn() {
            page_table.map(vpn, ppn, flags);
        } else {
            println!("[Kernel][mm_map]No free physical page available for mapping");
            result = -1;
            break;
        }

        sta = end_va.into();
        println!("[Kernel][mm_map]end sta = {}\n", usize::from(sta));
    }
    println!("[Kernel][mm_map] OK");
    result
}

pub fn mm_unmap(token: usize, start: usize, len: usize) -> isize {
    let mut page_table = PageTable::from_token(token);
    let mut result = 0;
    println!("[Kernel][mm_unmap]start = {}, len = {}", start, len);
    let mut sta = start;
    let end = start + len;
    while sta < end {
        let start_va = VirtAddr::from(sta);
        let mut vpn = start_va.floor();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));

        println!("[Kernel][mm_unmap]start_va = {}", usize::from(start_va));
        println!("[Kernel][mm_unmap]vpn = {}", usize::from(vpn));

        page_table.unmap(vpn);

        sta = end_va.into();
        println!("[Kernel][mm_unmap]end sta = {}\n", usize::from(sta));
    }
    println!("[Kernel][mm_unmap] OK");
    result
}

