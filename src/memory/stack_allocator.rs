use paging;
use paging::page::{Page, PageIter};
use memory::paging::ActivePageTable;
use mem_map::{MEM_PAGE_SIZE_BYTES, FrameAllocator};

#[derive(Debug)]
pub struct Stack {
    pub top: usize,
    pub bottom: usize,
}

impl Stack {
    fn new(top: usize, bottom: usize) -> Stack {
        assert!(top > bottom);
        Stack {
            top: top,
            bottom: bottom,
        }
    }
}

pub struct StackAllocator {
    range: PageIter
}

impl StackAllocator {
    pub fn new(range: PageIter) -> StackAllocator {
        StackAllocator { range }
    }

    pub fn alloc_stack<FA: FrameAllocator>(
                &mut self,
                active_table: &mut ActivePageTable,
                frame_allocator: &mut FA,
                size_in_pages: usize
            ) -> Option<Stack> {

        assert!(size_in_pages > 0);

        // Clone the range, since we only want to change it on success
        let mut range = self.range.clone();

        // try to allocate the stack pages and a guard page
        let guard_page = range.next();

        let stack_start = range.next();
        let stack_end = if size_in_pages == 1 {
            stack_start
        }
        else {
            // index starts at 0 and we have already allocated the start page
            range.nth(size_in_pages-2)
        };

        if let (Some(_), Some(start), Some(end)) = (guard_page, stack_start, stack_end) {
            // Success

            // Write back updated range
            self.range = range;

            // Map stack pages to physical frames
            for page in Page::range_inclusive(start, end) {
                active_table.map(page, paging::entry::WRITABLE, frame_allocator);
            }

            // Create a new stack
            let new_top = end.start_address() + MEM_PAGE_SIZE_BYTES;
            Some(Stack::new(new_top, start.start_address()))
        }
        else {
            // Not enough pages
            None
        }
    }
}
