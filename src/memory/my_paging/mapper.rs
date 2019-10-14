//! https://os.phil-opp.com/paging-implementation/#map-the-complete-physical-memory
//! https://wiki.osdev.org/Page_Tables
//!
//! Uses only 2MiB huge pages.

use core::mem;
use core::ptr;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging as pg;
use x86_64::structures::paging::page_table::{PageTable, PageTableEntry, PageTableFlags as Flags};
use x86_64::PhysAddr;

use super::super::prelude::*;

macro_rules! try_bool {
    ($expr:expr) => {{
        if !$expr {
            return false;
        }
    }};
}

/// Numeric value of `PT_PADDR` for static assertions
const PT_PADDR_INT: u64 = 0x10_000_000;

/// Physical address of the page table area
/// This pointer itself points to P4 table.
const PT_PADDR: PhysAddr = unsafe { PhysAddr::new_unchecked(PT_PADDR_INT) };

// Require P2 alignment
static_assertions::const_assert!(PT_PADDR_INT % 0x1_000_000 == 0);

/// Numeric value of `PT_VADDR` for static assertions
const PT_VADDR_INT: u64 = 0x10_000_000;

/// Page tables are mapped starting from this virtual address.
/// This pointer itself points to P4 table.
const PT_VADDR: VirtAddr = unsafe { VirtAddr::new_unchecked_raw(PT_VADDR_INT) };

// Require P2 alignment
static_assertions::const_assert!(PT_VADDR_INT % 0x1_000_000 == 0);

/// Size of 2MiB huge page, in bytes
const HUGE_PAGE_SIZE: u64 = 0x200_000;

/// Maximum number of page tables
const MAX_PAGE_TABLES: u64 = HUGE_PAGE_SIZE / 0x1000;

macro_rules! get_page {
    ($index:literal) => {{
        Page::from_start_address(PT_VADDR + 0x1000u64 * $index).unwrap()
    }};
}

macro_rules! frame_addr {
    ($index:expr) => {{
        PT_PADDR + 0x1000u64 * $index
    }};
}

macro_rules! frame {
    ($index:expr) => {{
        PhysFrame::from_start_address(frame_addr!($index)).unwrap()
    }};
}

macro_rules! pt_flags {
    (4) => {{
        Flags::PRESENT | Flags::WRITABLE
    }};
    (3) => {{
        Flags::PRESENT | Flags::WRITABLE
    }};
    (2) => {{
        Flags::PRESENT | Flags::WRITABLE | Flags::HUGE_PAGE
    }};
}

/// # Paging manager
/// Uses one huge page for page tables themselves, allowing 0x200 tables,
/// totalling to 0x1fe * 0x200 * 0x200_000 = 0x7_f80_000_000 = 510 GiB of ram
pub struct PageMap {
    /// Physical address of the page table
    p4_addr: PhysAddr,
    /// Next table will be placed to `PAGE_TABLE_AREA + PAGE_SIZE * page_count`,
    /// where `PAGE_SIZE` is `0x1000`.
    page_count: u64,
}

impl PageMap {
    /// Initializes page table structure. Requires that frame allocator
    /// provides properly mapped frames.
    ///
    /// This function works under a (tested) assumption that PAGE_TABLE_AREA is
    /// P2 aligned. This way, only one P2 is required to create all the entries.
    ///
    /// # Safety
    ///
    /// This function must only be called once
    #[must_use]
    pub unsafe fn init() -> Self {
        // We need to create and map one table for each level
        let mut p4_table: &mut PageTable = unsafe { &mut *frame_addr!(0).as_mut_ptr() };
        let mut p3_table: &mut PageTable = unsafe { &mut *frame_addr!(1).as_mut_ptr() };
        let mut p2_table: &mut PageTable = unsafe { &mut *frame_addr!(2).as_mut_ptr() };

        // Zero the tables
        p4_table.zero();
        p3_table.zero();
        p2_table.zero();

        // Map P4->P3 and P3->P2
        let page = get_page!(0);
        p4_table[page.p4_index()].set_addr(frame_addr!(1), pt_flags!(4));
        p3_table[page.p3_index()].set_addr(frame_addr!(2), pt_flags!(3));

        // Use P2 to map page table area
        p2_table[page.p2_index()].set_addr(frame_addr!(0), pt_flags!(2));

        Self {
            p4_addr: frame_addr!(0),
            page_count: 3,
        }
    }

    /// Sets the current table as the active page table
    #[inline(always)]
    pub(super) unsafe fn activate(&self) {
        bochs_magic_bp!();
        Cr3::write(
            pg::PhysFrame::<pg::Size4KiB>::from_start_address(self.p4_addr).expect("Misaligned P4"),
            Cr3Flags::empty(),
        );
        bochs_magic_bp!();
    }

    pub unsafe fn map_to(
        &mut self,
        page: Page,
        frame: PhysFrame,
        flags: Flags,
    ) -> MapperFlush<pg::Size2MiB> {
        let i4 = page.p4_index();
        let i3 = page.p3_index();
        let i2 = page.p2_index();

        // Resolve the P2 table, filling in possibly missing higher-level tables,
        // and then map the actual address

        let p4: &mut PageTable = &mut *self.p4_addr.as_mut_ptr();

        if p4[i4].is_unused() {
            // P3 table missing
            panic!("P3 MISSING"); // TODO: Over 512GiB virtual address required?
        }
        let p3: &mut PageTable = &mut *p4[i4].addr().as_mut_ptr();

        let p2: &mut PageTable = if p3[i3].is_unused() {
            // P2 table missing
            let addr = frame_addr!(self.page_count);
            self.page_count += 1;
            p3[i3].set_addr(addr, pt_flags!(3));
            let mut table: &mut PageTable = unsafe { &mut *addr.as_mut_ptr() };
            table.zero();
            table
        } else {
            &mut *p3[i3].addr().as_mut_ptr()
        };

        // Map the address
        p2[i2].set_addr(frame.start_address(), flags | Flags::HUGE_PAGE);

        MapperFlush::new(page)
    }

    pub unsafe fn identity_map(
        &mut self,
        frame: PhysFrame,
        flags: Flags,
    ) -> MapperFlush<pg::Size2MiB> {
        let page =
            Page::from_start_address(VirtAddr::new_unchecked(frame.start_address().as_u64()))
                .expect("Invalid physical address: no corresponding virtual address");
        self.map_to(page, frame, flags)
    }
}

/// TLB flush infoirmation for a page
#[derive(Debug)]
#[must_use = "Page Table changes must be flushed or ignored."]
pub struct MapperFlush<S: PageSize>(pg::Page<S>);

impl<S: PageSize> MapperFlush<S> {
    /// Create a new flush promise
    fn new(page: pg::Page<S>) -> Self {
        MapperFlush(page)
    }

    /// Flush the page from the TLB
    pub fn flush(self) {
        x86_64::instructions::tlb::flush(self.0.start_address());
    }

    /// Explicitly skip flushing the change
    pub fn ignore(self) {}
}
