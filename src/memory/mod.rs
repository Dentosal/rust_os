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
pub mod paging;
pub mod prelude;
mod stack_allocator;
mod utils;

pub use self::constants::*;
pub use self::prelude::*;
pub use self::stack_allocator::Stack;
pub use self::utils::*;

use d7alloc::{HEAP_SIZE, HEAP_START};

pub struct MemoryController {
    active_table_addr: PhysAddr,
    pub frame_allocator: frame_allocator::Allocator,
    stack_allocator: stack_allocator::StackAllocator,
}

impl MemoryController {
    fn paging<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self, &mut pg::OffsetPageTable) -> T,
    {
        let mut p4_table = unsafe { &mut *(self.active_table_addr.as_u64() as *mut PageTable) };
        f(
            self,
            &mut pg::OffsetPageTable::new(p4_table).expect("Invalid page table"),
        )
    }

    pub fn alloc_stack(&mut self, size_in_pages: usize) -> Option<Stack> {
        self.paging(|mem_ctrl, active_table| {
            let &mut MemoryController {
                ref mut frame_allocator,
                ref mut stack_allocator,
                ..
            } = mem_ctrl;

            stack_allocator.alloc_stack(active_table, frame_allocator, size_in_pages)
        })
    }

    // pub fn alloc_executable(&mut self, size_in_pages: usize) -> MemoryArea {
    //     use alloc::prelude::v1::Vec;

    //     let &mut MemoryController {
    //         ref mut active_table,
    //         ref mut frame_allocator,
    //         ref mut virtual_allocator,
    //         ..
    //     } = self;

    //     let mut frames: Vec<Frame> = (0..size_in_pages)
    //         .map(|_| {
    //             frame_allocator
    //                 .allocate_frame()
    //                 .expect("Could not allocate frame")
    //         })
    //         .collect();

    //     // Allocate contiguous virtual address block
    //     // TODO: After proper context switching, map to constant address
    //     // TODO: After above and higher half kernel, map to zero

    //     // for frame in frames {
    //     //     active_table.map_to(frame, PageTableFlags::zero(), &mut frame_allocator);
    //     // }

    //     unimplemented!();
    // }
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

    bochs_magic_bp!();

    let mut frame_allocator = unsafe { self::frame_allocator::Allocator::new(memory_map) };

    // Remap kernel and get page table
    let mut active_table_addr = paging::init(&mut frame_allocator, elf_metadata);
    let mut p4_table = unsafe { &mut *(active_table_addr.as_u64() as *mut PageTable) };
    let mut active_table = pg::RecursivePageTable::new(p4_table).expect("Invalid page table");

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
                    Flags::WRITABLE | Flags::NO_EXECUTE,
                    &mut frame_allocator,
                )
                .expect("Could not map page")
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
        active_table_addr,
        frame_allocator,
        stack_allocator,
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
        unreachable!();
    }
}
