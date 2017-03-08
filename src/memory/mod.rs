mod stack_allocator;

use paging;
use paging::page::Page;
use mem_map;
use elf_parser;

use hole_list_allocator::{HEAP_START, HEAP_SIZE};

pub use self::stack_allocator::Stack;

pub struct MemoryController {
    active_table: paging::ActivePageTable,
    frame_allocator: mem_map::BitmapAllocator,
    stack_allocator: stack_allocator::StackAllocator,
}

impl MemoryController {
    pub fn alloc_stack(&mut self, size_in_pages: usize) -> Option<Stack> {
        let &mut MemoryController {
            ref mut active_table,
            ref mut frame_allocator,
            ref mut stack_allocator
        } = self;
        stack_allocator.alloc_stack(active_table, frame_allocator, size_in_pages)
    }
}

pub fn init() -> MemoryController {
    // receive raw kernel elf image data before we allow overwriting it
    let elf_metadata = unsafe {elf_parser::parse_kernel_elf()};

    // frame allocator
    mem_map::create_memory_bitmap();

    // initalize paging system
    unsafe {
        paging::enable_nxe();
        paging::enable_write_protection();
    }

    let mut frame_allocator = ALLOCATOR!();
    let mut active_table: paging::page_table::ActivePageTable = paging::remap_kernel(&mut frame_allocator, elf_metadata);

    let heap_start_page = Page::containing_address(HEAP_START);
    let heap_end_page = Page::containing_address(HEAP_START + HEAP_SIZE - 1);
    for page in Page::range_inclusive(heap_start_page, heap_end_page) {
        // TODO: remove VVVV
        rprintln!("{:?}", page); // XXX: only this side effect works???
        active_table.map(page, paging::entry::WRITABLE, &mut frame_allocator);
    }

    let stack_allocator = {
        let stack_alloc_start = heap_end_page + 1;
        let stack_alloc_end = stack_alloc_start + 100;
        let stack_alloc_range = Page::range_inclusive(stack_alloc_start, stack_alloc_end);
        stack_allocator::StackAllocator::new(stack_alloc_range)
    };

    MemoryController {
        active_table: active_table,
        frame_allocator: frame_allocator,
        stack_allocator: stack_allocator
    }
}
