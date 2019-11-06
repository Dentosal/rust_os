//! Kernel stack for system calls

use core::ptr;
use x86_64::structures::paging::PageTableFlags as Flags;

use crate::filesystem::staticfs::read_file;
use crate::memory::{self, prelude::*};

use super::super::MemoryController;

/// Creates and maps the system call stack.
/// There is no need to zero the memory, as it will not be read,
/// and it is inaccessible for user processes.
pub fn init() {
    memory::configure(|mem_ctrl: &mut MemoryController| unsafe {
        let frame = mem_ctrl
            .frame_allocator
            .allocate_frame()
            .expect("Could not allocate frame");

        mem_ctrl
            .page_map
            .map_to(
                PT_VADDR,
                Page::from_start_address(memory::SYSCALL_STACK).unwrap(),
                frame,
                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
            )
            .flush();
    });
}
