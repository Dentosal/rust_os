mod area;
mod constants;
mod frame_allocator;
mod map;
pub mod prelude;
mod utils;

use x86_64::structures::paging::Size4KiB;
use x86_64::structures::paging::{OffsetPageTable, PageTable, PageTableFlags as Flags};
use x86_64::{PhysAddr, VirtAddr};

use crate::elf_parser;

use self::prelude::*;
pub use self::utils::*;

/// Creates an example mapping for the given page to frame `0xb8000`.
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;
    use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PhysFrame};

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe { mapper.map_to(page, frame, flags, frame_allocator) };
    map_to_result.expect("map_to failed").flush();
}

pub fn init() {
    // Receive raw kernel elf image data before it's overwritten
    let elf_metadata = unsafe { elf_parser::parse_kernel_elf() };

    // Receive memory map before it's overwritten
    let memory_map = map::load_memory_map();

    let mut frame_allocator = unsafe { self::frame_allocator::Allocator::new(memory_map) };

    let phys_mem_offset = VirtAddr::new(0x0);
    let mut mapper = unsafe { create_table(phys_mem_offset) };

    let page = Page::containing_address(VirtAddr::new(0));
    create_example_mapping(page, &mut mapper, &mut frame_allocator);

    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e) };
}

pub struct MemoryController;

impl MemoryController {
    fn paging<F, T>(&mut self, _f: ()) -> T {
        unimplemented!()
    }

    pub fn alloc_stack(&mut self, size_in_pages: usize) -> Option<()> {
        unimplemented!()
    }
}
