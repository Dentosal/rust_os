use alloc::vec::Vec;
use spin::Mutex;
use x86_64::structures::paging as pg;
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::structures::paging::{Mapper, PageTable};

use crate::elf_parser;

mod area;
mod constants;
pub mod dma_allocator;
mod frame_allocator;
mod map;
mod paging;
pub mod prelude;
mod stack_allocator;
mod utils;
pub mod virtual_allocator;

pub use self::constants::*;
pub use self::prelude::*;
pub use self::stack_allocator::Stack;
pub use self::utils::*;

use self::paging::PageMap;

use d7alloc::{HEAP_SIZE, HEAP_START};

pub struct MemoryController {
    pub active_table: PageMap,
    pub frame_allocator: frame_allocator::Allocator,
    stack_allocator: stack_allocator::StackAllocator,
    virtual_allocator: virtual_allocator::VirtualAllocator,
}

impl MemoryController {
    pub fn alloc_stack(&mut self, size_in_pages: usize) -> Option<Stack> {
        self.stack_allocator.alloc_stack(
            &mut self.active_table,
            &mut self.frame_allocator,
            size_in_pages,
        )
    }

    /// Allocates a set of physical memory frames
    pub fn alloc_frames(&mut self, size_in_pages: usize) -> Vec<PhysFrame> {
        (0..size_in_pages)
            .map(|_| {
                self.frame_allocator
                    .allocate_frame()
                    .expect("Could not allocate frame")
            })
            .collect()
    }

    /// Allocates a contiguous virtual memory area
    pub fn alloc_virtual_area(&mut self, size_in_pages: u64) -> virtual_allocator::Area {
        let start = self.virtual_allocator.allocate(size_in_pages);
        virtual_allocator::Area::new_pages(start, size_in_pages)
    }

    /// Allocate a contiguous virtual address block,
    /// and page-map it with the given flags
    pub fn alloc_pages(&mut self, size_in_pages: usize, flags: Flags) -> virtual_allocator::Area {
        let mut frames: Vec<PhysFrame> = self.alloc_frames(size_in_pages);

        let start = self.virtual_allocator.allocate(size_in_pages as u64);
        let mut page_index = 0;

        for frame in frames {
            unsafe {
                self.active_table
                    .map_to(
                        Page::from_start_address(start + (page_index as u64) * PAGE_SIZE_BYTES)
                            .unwrap(),
                        frame,
                        flags,
                    )
                    .flush();
            }
            page_index += 1;
        }

        virtual_allocator::Area::new_pages(start, page_index)
    }
}

pub fn init() {
    // Receive raw kernel elf image data before it's overwritten
    let elf_metadata = unsafe { elf_parser::parse_kernel_elf() };

    // Receive memory map before it's overwritten
    let memory_map = map::load_memory_map();

    // initalize paging system
    unsafe {
        paging::enable_nxe();
        paging::enable_write_protection();
    }

    // Remap kernel and get page table
    let mut active_table = unsafe { paging::init(elf_metadata) };

    // Initialize frame allocator
    let mut frame_allocator = unsafe { self::frame_allocator::Allocator::new(memory_map) };

    // Identity map heap
    let heap_start_page = pg::Page::containing_address(VirtAddr::new(HEAP_START));
    let heap_end_page = pg::Page::containing_address(VirtAddr::new(HEAP_START + HEAP_SIZE - 1));
    for page in pg::Page::range_inclusive(heap_start_page, heap_end_page) {
        let frame = frame_allocator.allocate_frame().expect("Out of memory");
        unsafe {
            active_table
                .map_to(
                    page,
                    frame,
                    Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                )
                .flush();
        }
    }

    let stack_allocator = {
        let stack_alloc_start = heap_end_page + 1;
        let stack_alloc_end = stack_alloc_start + 100;
        let stack_alloc_range = pg::Page::range_inclusive(stack_alloc_start, stack_alloc_end);
        stack_allocator::StackAllocator::new(stack_alloc_range)
    };

    let mem_ctrl = MemoryController {
        active_table,
        frame_allocator,
        stack_allocator,
        virtual_allocator: virtual_allocator::VirtualAllocator::new(),
    };

    let mut guard = MEM_CTRL_CONTAINER.lock();
    *guard = Some(mem_ctrl);
}

/// Static memory controller
static MEM_CTRL_CONTAINER: Mutex<Option<MemoryController>> = Mutex::new(None);

pub fn configure<F, T>(mut f: F) -> T
where
    F: FnMut(&mut MemoryController) -> T,
{
    let mut guard = MEM_CTRL_CONTAINER.lock();
    if let Some(ref mut mem_ctrl) = *guard {
        f(mem_ctrl)
    } else {
        unreachable!("Memory controller missing");
    }
}
