//! Kernel stack for system calls

use x86_64::structures::paging::PageTableFlags as Flags;

use crate::memory::{paging::PAGE_MAP, phys, prelude::*, SYSCALL_STACK};

/// Creates and maps the system call stack.
/// There is no need to zero the memory, as it will not be read,
/// and it is inaccessible for user processes.
pub unsafe fn init() {
    let frame = phys::allocate(PAGE_LAYOUT)
        .expect("Could not allocate frame")
        .leak();

    let mut page_map = PAGE_MAP.try_lock().unwrap();
    page_map
        .map_to(
            PT_VADDR,
            Page::from_start_address(SYSCALL_STACK).unwrap(),
            PhysFrame::from_start_address_unchecked(frame.start()),
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        )
        .flush();
}
