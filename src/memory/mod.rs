use paging;
use paging::page::Page;
use mem_map;
use elf_parser;

use hole_list_allocator::{HEAP_START, HEAP_SIZE};

pub fn init() {
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

    rprintln!("IT WORKED!");
}
