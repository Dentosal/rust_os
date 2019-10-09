//! https://os.phil-opp.com/paging-implementation/
//! http://os.phil-opp.com/entering-longmode.html#set-up-identity-paging
//! http://wiki.osdev.org/Paging
//! http://pages.cs.wisc.edu/~remzi/OSTEP/vm-paging.pdf
//!
//! This module only supports 1KiB pages (<- obsolete?)

use core::mem;
use core::ptr;
use x86_64::structures::paging::page_table::{PageTable, PageTableEntry, PageTableFlags as Flags};
use x86_64::ux;

use super::super::prelude::*;

const REC_INDEX: ux::u9 = ux::u9::MAX;
const REC_PAGE: VirtAddr = unsafe { VirtAddr::new_unchecked_raw((1u64 << 63) - 0x1000) };

fn p3_ptr(page: Page) -> *mut PageTable {
    p3_page(page).start_address().as_mut_ptr()
}

fn p3_page(page: Page) -> Page {
    Page::from_page_table_indices(REC_INDEX, REC_INDEX, REC_INDEX, page.p4_index())
}

fn p2_ptr(page: Page) -> *mut PageTable {
    p2_page(page).start_address().as_mut_ptr()
}

fn p2_page(page: Page) -> Page {
    Page::from_page_table_indices(REC_INDEX, REC_INDEX, page.p4_index(), page.p3_index())
}

fn p1_ptr(page: Page) -> *mut PageTable {
    p1_page(page).start_address().as_mut_ptr()
}

fn p1_page(page: Page) -> Page {
    Page::from_page_table_indices(REC_INDEX, page.p4_index(), page.p3_index(), page.p2_index())
}

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

        rforce_unlock!();
        rprintln!("ENTRYFRAME {:#?}", (entry.clone(), frame.clone()));

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

unsafe fn map_to<A>(
    p4_table: &mut PageTable,
    page: Page,
    frame: PhysFrame,
    flags: Flags,
    allocator: &mut A,
) where
    A: FrameAllocator<PageSizeType>,
{
    let p3_page = p3_page(page);
    let p3_table = create_next_table(&mut p4_table[page.p4_index()], p3_page, allocator);

    let p2_page = p2_page(page);
    let p2_table = create_next_table(&mut p3_table[page.p3_index()], p2_page, allocator);

    let p1_page = p1_page(page);
    let p1_table = create_next_table(&mut p2_table[page.p2_index()], p1_page, allocator);

    if !p1_table[page.p1_index()].is_unused() {
        panic!("Page already mapped");
    }

    p1_table[page.p1_index()].set_frame(frame, flags);
}

unsafe fn identity_map<A>(
    p4_table: &mut PageTable,
    frame: PhysFrame,
    flags: Flags,
    allocator: &mut A,
) where
    A: FrameAllocator<PageSizeType>,
{
    map_to(
        p4_table,
        Page::from_start_address(VirtAddr::new(frame.start_address().as_u64())).unwrap(),
        frame,
        flags,
        allocator,
    );
}

// Create new page table using 4KiB pages
// * Identity map first 1GiB (0x40_000 * 0x1_000)
// * Recursively map last entry to the page table itself
pub fn create_table<'a, A>(allocator: &mut A) -> PhysAddr
where
    A: FrameAllocator<PageSizeType>,
{
    // Identity mapping first 1GiB uses with 4KiB pages uses 0x40_000 P1 entries,
    // which is exactly 0x200 P2 entries, which is exactly 1 P3 entry.

    let p4_frame = allocator.allocate_frame().expect("Alloc failed");
    let mut p4_table = unsafe { &mut *(p4_frame.start_address().as_u64() as *mut PageTable) };
    p4_table.zero();

    let start_frame = PhysFrame::from_start_address(PhysAddr::new(0)).unwrap();
    let end_frame = PhysFrame::containing_address(PhysAddr::new(0x40_000_000 - 1));
    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        unsafe {
            identity_map(p4_table, frame, Flags::PRESENT | Flags::WRITABLE, allocator);
        }
    }

    // Recursively map the last the last page in p4
    // http://os.phil-opp.com/modifying-page-tables.html#implementation
    unsafe {
        map_to(
            p4_table,
            Page::from_start_address(REC_PAGE).unwrap(),
            p4_frame,
            Flags::PRESENT | Flags::WRITABLE,
            allocator,
        );
    }

    p4_frame.start_address()
}
