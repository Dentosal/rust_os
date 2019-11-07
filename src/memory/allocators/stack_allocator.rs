use x86_64::structures::paging as pg;
use x86_64::structures::paging::page_table::PageTableFlags as Flags;

use super::super::paging::PageMap;
use super::super::prelude::*;

#[derive(Debug)]
pub struct Stack {
    pub top: PhysAddr,
    pub bottom: PhysAddr,
}

impl Stack {
    fn new(top: PhysAddr, bottom: PhysAddr) -> Stack {
        assert!(top > bottom);
        Stack { top, bottom }
    }
}

pub struct StackAllocator {
    range: pg::page::PageRangeInclusive<pg::Size2MiB>,
}

impl StackAllocator {
    pub fn new(range: pg::page::PageRangeInclusive<pg::Size2MiB>) -> Self {
        Self { range }
    }

    /// Requires that the kernel page table is active
    pub fn alloc_stack<A: pg::FrameAllocator<pg::Size2MiB>>(
        &mut self, page_map: &mut PageMap, frame_allocator: &mut A, size_in_pages: usize,
    ) -> Option<Stack> {
        assert!(size_in_pages > 0);

        // Clone the range, since we only want to change it on success
        let mut range = self.range.clone();

        // try to allocate the stack pages and a guard page
        let guard_page = range.next();

        let stack_start = range.next();
        let stack_end = if size_in_pages == 1 {
            stack_start
        } else {
            // index starts at 0 and we have already allocated the start page
            range.nth(size_in_pages - 2)
        };

        if let (Some(_), Some(start), Some(end)) = (guard_page, stack_start, stack_end) {
            // Success

            // Write back updated range
            self.range = range;

            // Map stack pages to physical frames
            for page in Page::range_inclusive(start, end) {
                let frame = frame_allocator
                    .allocate_frame()
                    .expect("Could not allocate stack frame");

                unsafe {
                    page_map
                        .map_to(
                            PT_VADDR,
                            page,
                            frame,
                            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                        )
                        .flush();
                }
            }

            // Create a new stack
            let new_top = end.start_address() + Page::SIZE;
            // TODO: Check that virtual and physical addresses match, or map
            Some(Stack::new(
                PhysAddr::new(new_top.as_u64()),
                PhysAddr::new(start.start_address().as_u64()),
            ))
        } else {
            // Not enough pages
            None
        }
    }
}
