use core::ptr;
use x86_64::structures::paging::FrameAllocator;

use crate::elf_parser::*;
use crate::memory::prelude::*;
use crate::memory::{self, MemoryController, Page, PhysFrame};
use crate::staticfs;

use alloc::prelude::v1::Vec;

pub fn load_module(path: &str) -> Option<()> {
    let bytes = staticfs::read_file(path)?;

    let size_pages =
        memory::page_align(PhysAddr::new(bytes.len() as u64), true).as_u64() / Page::SIZE;

    // Allocate load buffer
    let area = memory::configure(|mem_ctrl| mem_ctrl.alloc_pages(size_pages as usize));

    // Store the file to buffer
    let base: *mut u8 = area.start.as_mut_ptr();
    let mut it = bytes.into_iter();
    for page_offset in 0..size_pages {
        for byte_offset in 0..PAGE_SIZE_BYTES {
            let i = page_offset * PAGE_SIZE_BYTES + byte_offset;
            unsafe {
                ptr::write(base.offset(i as isize), it.next().unwrap_or(0));
            }
        }
    }

    let elf = unsafe { parse_elf(base as usize) };

    // Set up page tables for the new process
    // Prepare virtual address space for the process

    rprintln!("?? {:?}", elf);
    unimplemented!(); // TODO

    Some(())
}
