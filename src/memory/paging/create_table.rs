//! https://os.phil-opp.com/paging-implementation/
//! http://os.phil-opp.com/entering-longmode.html#set-up-identity-paging
//! http://wiki.osdev.org/Paging
//! http://pages.cs.wisc.edu/~remzi/OSTEP/vm-paging.pdf
//!
//! This module only supports 1KiB pages (<- obsolete?)

use core::mem;
use core::ptr;
use x86_64::structures::paging::page_table::{PageTable, PageTableEntry, PageTableFlags as Flags};
use x86_64::structures::paging::{Mapper, OffsetPageTable};

use super::super::prelude::*;

/// Create the page table of the next level if needed.
///
/// If the passed entry is unused, a new frame is allocated from the given allocator, zeroed,
/// and the entry is updated to that address. If the passed entry is already mapped, the next
/// table is returned directly.
///
/// The `next_page_table` page must be the page of the next page table in the hierarchy.
unsafe fn create_next_table<'b, A>(
    entry: &'b mut PageTableEntry,
    next_table_page: Page,
    allocator: &mut A,
) -> &'b mut PageTable
where
    A: FrameAllocator<PageSizeType>,
{
    let create_new = entry.is_unused();

    if create_new {
        let frame = allocator.allocate_frame().expect("Alloc failed");
        entry.set_frame(frame, Flags::PRESENT | Flags::WRITABLE);
    }

    if entry.flags().contains(Flags::HUGE_PAGE) {
        panic!("Cannot map below huge pages");
    }

    let page_table_ptr = next_table_page.start_address().as_mut_ptr();
    let page_table: &mut PageTable = &mut *page_table_ptr;

    if create_new {
        page_table.zero();
    }

    page_table
}

// Create new page table using 4KiB pages, identity mapping the first 1GiB (0x40_000 * 0x1_000)
pub fn create_table<'a, A>(allocator: &mut A) -> PhysAddr
where
    A: FrameAllocator<PageSizeType>,
{
    // Identity mapping first 1GiB uses with 4KiB pages uses 0x40_000 P1 entries,
    // which is exactly 0x200 P2 entries, which is exactly 1 P3 entry.

    let p4_frame = allocator.allocate_frame().expect("Alloc failed");
    let mut p4_table = unsafe { &mut *(p4_frame.start_address().as_u64() as *mut PageTable) };
    p4_table.zero();

    // let mut table = unsafe { OffsetPageTable::new(p4_table, VirtAddr::new(0x0)) };

    // let start_frame = PhysFrame::from_start_address(PhysAddr::new(0x0)).unwrap();
    // let end_frame = PhysFrame::containing_address(PhysAddr::new(0x40_000_000 - 1));
    // for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
    //     unsafe {
    //         let flusher = table
    //             .identity_map(frame, Flags::PRESENT | Flags::WRITABLE, allocator)
    //             .expect("Mapping failed");
    //         flusher.ignore();
    //     }
    // }
    p4_frame.start_address()
}
