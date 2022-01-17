use alloc::vec::Vec;
use core::alloc::Layout;
use core::mem;
use core::ptr;
use core::slice;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use x86_64::structures::paging as pg;
use x86_64::structures::paging::Mapper;
use x86_64::structures::paging::PageTableFlags as Flags;

mod allocators;
mod area;
mod map;
mod utils;

pub mod constants;
pub mod paging;
pub mod prelude;
pub mod process_common_code;

pub mod phys;
pub mod rust_heap;
pub mod virt;

use crate::multitasking::Process;
use crate::util::elf_parser;

pub use self::allocators::*;
pub use self::prelude::*;

use self::paging::PageMap;

/// Flag to determine if allocations are available
static ALLOCATOR_READY: AtomicBool = AtomicBool::new(false);

/// Checks if allocations are available
pub fn can_allocate() -> bool {
    ALLOCATOR_READY.load(Ordering::Acquire)
}

/// # Safety: the caller must ensure that this is called only once
pub unsafe fn init() {
    // Receive raw kernel elf image data before it's overwritten
    let elf_metadata = unsafe { elf_parser::parse_kernel_elf() };

    // Receive memory map before it's overwritten
    let memory_info = map::load_memory_map();

    // initalize paging system
    unsafe {
        paging::enable_nxe();
        paging::enable_write_protection();
    }

    // Remap kernel and get page table
    unsafe { paging::init(elf_metadata) };

    // Map all physical memory to the higher half
    let a = PhysFrame::from_start_address(PhysAddr::zero()).unwrap();
    let b = PhysFrame::containing_address(PhysAddr::new(memory_info.max_memory));
    {
        let mut page_map = paging::PAGE_MAP.try_lock().unwrap();
        for frame in PhysFrame::range_inclusive(a, b) {
            let f_u64 = frame.start_address().as_u64();
            let page = Page::from_start_address(VirtAddr::new(
                constants::HIGHER_HALF_START.as_u64() + f_u64,
            ))
            .unwrap();

            unsafe {
                page_map
                    .map_to(
                        PT_VADDR,
                        page,
                        frame,
                        Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                    )
                    .flush(); // TODO: flush all at once
            }
        }
    }

    // Initialize frame allocator
    self::phys::init(memory_info.allocatable);

    // Set allocations available
    ALLOCATOR_READY.store(true, Ordering::Release);
    log::debug!("Allocation is now available");

    // InitRD
    crate::initrd::init(elf_metadata);

    // Prepare a kernel stack for syscalls
    syscall_stack::init();

    // Load process switching code
    process_common_code::init();
}

/// Convert PhysAddr to VirtAddr, only accessible in by the kernel,
/// i.e. requires kernel page tables to be active.
///
/// This uses the constant always-available higher-half physical memory mapping.
pub fn phys_to_virt(offset: PhysAddr) -> VirtAddr {
    VirtAddr::new(
        constants::HIGHER_HALF_START
            .as_u64()
            .checked_add(offset.as_u64())
            .expect("phys_to_virt overflow"),
    )
}
