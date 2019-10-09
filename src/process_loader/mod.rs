use core::ptr;
use x86_64::structures::paging::FrameAllocator;

use crate::memory::prelude::*;
use crate::memory::{self, MemoryController, Page, PhysFrame};
use crate::staticfs;

use alloc::prelude::v1::Vec;

pub fn load_module(path: &str) -> Option<()> {
    let bytes = staticfs::read_file(path)?;

    let size_pages =
        memory::page_align(PhysAddr::new(bytes.len() as u64), true).as_u64() / Page::SIZE;

    // Allocate load buffer
    let mut frames: Vec<PhysFrame> = memory::configure(|mem_ctrl| {
        (0..size_pages)
            .map(|_| {
                mem_ctrl
                    .frame_allocator
                    .allocate_frame()
                    .expect("Could not allocate frame")
            })
            .collect()
    });

    // Store the file to buffer
    // TODO: Load directly to the buffer, and not through a Vec
    for (frame, data) in frames.iter().zip(bytes.chunks(Page::SIZE as usize)) {
        let base = frame.start_address().as_u64() as *mut u8;

        for (i, &byte) in data.iter().enumerate() {
            unsafe {
                ptr::write(base.offset(i as isize), byte);
            }
        }
    }

    // Set up page tables for the new process

    // Prepare virtual address space for the process

    memory::configure(|ctrl| {
        // ctrl.alloc_executable(size_pages);
    });

    Some(())

    // for byte in bytes {

    // }
}
