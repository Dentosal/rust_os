use core::alloc::Layout;
use x86_64::structures::paging as pg;
use x86_64::structures::paging::page_table::PageTableFlags as Flags;

use crate::memory::{paging::PAGE_MAP, phys, prelude::*, virt};

#[derive(Debug)]
pub struct Stack {
    pub top: VirtAddr,
    pub bottom: VirtAddr,
}

impl Stack {
    fn new(top: VirtAddr, bottom: VirtAddr) -> Stack {
        assert!(top > bottom);
        Stack { top, bottom }
    }
}

/// Allocates a new stack, including a guard page, and maps it.
/// Requires that the kernel page table is active.
/// The stacks allocated by this can never be deallocated.
pub fn alloc_stack(size_in_pages: usize) -> Stack {
    assert!(size_in_pages > 0);
    // Allocate virtual addresses
    let v = virt::allocate(size_in_pages + 1);

    // Allocate and map the physical frames (not the guard page)
    let start_page = Page::from_start_address(v.start + PAGE_SIZE_BYTES).unwrap();
    let end_page =
        Page::from_start_address(v.start + size_in_pages * PAGE_SIZE_BYTES as usize).unwrap();

    let mut page_map = PAGE_MAP.lock();
    for page in Page::range_inclusive(start_page, end_page) {
        let frame = phys::allocate(PAGE_LAYOUT)
            .expect("Could not allocate stack frame")
            .leak();

        unsafe {
            page_map
                .map_to(
                    PT_VADDR,
                    page,
                    PhysFrame::from_start_address_unchecked(frame.start()),
                    Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                )
                .flush();
        }
    }

    // Create a new stack
    Stack::new(
        end_page.start_address() + Page::SIZE,
        start_page.start_address(),
    )
}
