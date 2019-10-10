//! https://wiki.osdev.org/Page_Tables

use core::mem;
use core::ptr;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging as pg;
use x86_64::structures::paging::page_table::{PageTable, PageTableEntry, PageTableFlags as Flags};
use x86_64::structures::paging::PhysFrame;
use x86_64::PhysAddr;

use super::super::prelude::*;

macro_rules! try_bool {
    ($expr:expr) => {{
        if !$expr {
            return false;
        }
    }};
}

/// Numeric value of `PAGE_TABLE_AREA` for static assertions
const PT_ADDR_INT: u64 = 0x10000000;

/// Page tables are mapped starting from this virtual address.
/// This pointer itself points to P4 table.
const PAGE_TABLE_AREA: VirtAddr = unsafe { VirtAddr::new_unchecked_raw(PT_ADDR_INT) };

// Require P2 alignment for PAGE_TABLE_AREA.
static_assertions::const_assert!(PT_ADDR_INT % 0x1_000_000 == 0);

/// Page entry count
const PAGE_ENTRIES: u32 = 0x200;

/// # Paging manager
/// The idea is following:
/// We keep three empty memory frames mapped (one for each intermediate level).
/// Any time we need to use more frames to map an area, those frames
/// are used to store the new tables. Then new frames are mapped for
/// the next time.
///
/// Currently just leaks virtual memory forever. This will not be an issue soon.
/// TODO: ^ Implement bookkeeping of freed sections of virtual address space,
///         and reuse the freed blocks.
pub struct PageMap {
    /// Physical address of the page table
    p4_addr: PhysAddr,
    /// Next frame for each table level [P1,P2,P3]. NOT ZEROED YET
    next_frame_for_level: [PhysFrame; 3],
    /// Next table will be placed to `PAGE_TABLE_AREA + PAGE_SIZE * page_count`,
    /// where `PAGE_SIZE` is `0x1000`.
    page_count: u64,
}

impl PageMap {
    /// Initializes page table structure. Requires that frame allocator
    /// provides properly mapped frames.
    ///
    /// This function works under (tested) assumption that PAGE_TABLE_AREA is
    /// P2 aligned. This way, only one P2 is required to create all the entries.
    ///
    /// # Safety
    ///
    /// This function must only be called once
    #[must_use]
    pub unsafe fn init<'a, A>(allocator: &mut A) -> Self
    where
        A: FrameAllocator<PageSizeType>,
    {
        let pt_flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE;

        macro_rules! get_page {
            ($index:literal) => {{
                Page::from_start_address(PAGE_TABLE_AREA + 0x1000u64 * $index)
                    .expect("Start address invalid")
            }};
        }

        // We need to create and map one table for each level
        let p4_frame = allocator.allocate_frame().expect("Alloc failed");
        let p4_addr = p4_frame.start_address();
        let mut p4_table: &mut PageTable = unsafe { &mut *p4_frame.start_address().as_mut_ptr() };
        p4_table.zero();

        let p3_frame = allocator.allocate_frame().expect("Alloc failed");
        let p3_addr = p3_frame.start_address();
        let mut p3_table: &mut PageTable = unsafe { &mut *p3_frame.start_address().as_mut_ptr() };
        p3_table.zero();

        let p2_frame = allocator.allocate_frame().expect("Alloc failed");
        let p2_addr = p2_frame.start_address();
        let mut p2_table: &mut PageTable = unsafe { &mut *p2_frame.start_address().as_mut_ptr() };
        p2_table.zero();

        let p1_frame = allocator.allocate_frame().expect("Alloc failed");
        let p1_addr = p1_frame.start_address();
        let mut p1_table: &mut PageTable = unsafe { &mut *p1_frame.start_address().as_mut_ptr() };
        p1_table.zero();

        // Map each frame to the lower one
        let page = get_page!(0);
        p4_table[page.p4_index()].set_addr(p3_addr, pt_flags);
        p3_table[page.p3_index()].set_addr(p2_addr, pt_flags);
        p2_table[page.p2_index()].set_addr(p1_addr, pt_flags);

        rprintln!("1T vr={:?}, ph={:?}", page.start_address(), p1_addr);
        rprintln!("2T vr={:?}, ph={:?}", page.start_address(), p2_addr);
        rprintln!("3T vr={:?}, ph={:?}", page.start_address(), p3_addr);
        rprintln!("4T vr={:?}, ph={:?}", page.start_address(), p4_addr);

        // Use P1 table to actually map the page table entries
        p1_table[get_page!(3).p1_index()].set_addr(p1_addr, pt_flags);
        p1_table[get_page!(2).p1_index()].set_addr(p2_addr, pt_flags);
        p1_table[get_page!(1).p1_index()].set_addr(p3_addr, pt_flags);
        p1_table[get_page!(0).p1_index()].set_addr(p4_addr, pt_flags);

        // Allocate and map three more entries for mapper
        let e0_frame = allocator.allocate_frame().expect("Alloc failed");
        let e1_frame = allocator.allocate_frame().expect("Alloc failed");
        let e2_frame = allocator.allocate_frame().expect("Alloc failed");

        let e0_addr = e0_frame.start_address();
        let e1_addr = e1_frame.start_address();
        let e2_addr = e2_frame.start_address();

        rprintln!("1E vr={:?}, ph={:?}", get_page!(4).start_address(), e0_addr);
        rprintln!("2E vr={:?}, ph={:?}", get_page!(5).start_address(), e1_addr);
        rprintln!("3E vr={:?}, ph={:?}", get_page!(6).start_address(), e2_addr);

        p1_table[get_page!(4).p1_index()].set_addr(e0_addr, pt_flags);
        p1_table[get_page!(5).p1_index()].set_addr(e1_addr, pt_flags);
        p1_table[get_page!(6).p1_index()].set_addr(e2_addr, pt_flags);

        Self {
            p4_addr,
            next_frame_for_level: [e0_frame, e1_frame, e2_frame],
            page_count: 7,
        }
    }

    /// Sets the current table as the active page table
    #[inline(always)]
    pub(super) unsafe fn activate(&self) {
        bochs_magic_bp!();
        Cr3::write(
            PhysFrame::from_start_address(self.p4_addr).expect("Misaligned P4"),
            Cr3Flags::empty(),
        );
        bochs_magic_bp!();
    }

    /// Allocate a new frame for a table, mapping it as required.
    /// This function does not flush TLB.
    #[must_use]
    pub unsafe fn allocate_table<'a, A>(&mut self, allocator: &mut A) -> PhysFrame
    where
        A: FrameAllocator<PageSizeType>,
    {
        let p0_addr = PAGE_TABLE_AREA + 0x1000 * self.page_count;
        let p0_page = Page::from_start_address(p0_addr).unwrap();
        let p0_frame = allocator.allocate_frame().expect("Alloc failed");
        self.page_count += 1;
        if !self.try_direct_map_to_0(p0_page, p0_frame) {
            let p1_addr = PAGE_TABLE_AREA + 0x1000 * self.page_count;
            let p1_page = Page::from_start_address(p1_addr).unwrap();
            let p1_frame = allocator.allocate_frame().expect("Alloc failed");
            self.page_count += 1;
            if !self.try_direct_map_to_1(p1_page, p1_frame) {
                panic!("Implement more levels");
            } else {
                assert!(self.try_direct_map_to_0(p0_page, p0_frame));
            }
        }

        rprintln!("NT vr={:?}, ph={:?}", p0_addr, p0_frame.start_address());

        p0_frame
    }

    /// # Sets P4->P3
    /// Tries to map a new table frame without allocating or using up frames.
    /// Returns true on success, and false if any entries were missing.
    /// This function does not flush TLB.
    /// Overwrites any existing mappings.
    #[must_use]
    unsafe fn try_direct_map_to_3(&self, page: Page, frame: PhysFrame) -> bool {
        let i4 = page.p4_index();

        // Resolve the P1 table
        let p4: &mut PageTable = &mut *self.p4_addr.as_mut_ptr();
        // Map the address
        p4[i4].set_addr(
            frame.start_address(),
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        );
        true
    }

    /// # Sets P3->P2
    /// Tries to map a new table frame without allocating or using up frames.
    /// Returns true on success, and false if any entries were missing.
    /// This function does not flush TLB.
    /// Overwrites any existing mappings.
    #[must_use]
    unsafe fn try_direct_map_to_2(&self, page: Page, frame: PhysFrame) -> bool {
        let i3 = page.p3_index();
        let i4 = page.p4_index();

        // Resolve the P1 table
        let p4: &mut PageTable = &mut *self.p4_addr.as_mut_ptr();
        try_bool!(!p4[i4].is_unused());
        let p3: &mut PageTable = &mut *p4[i4].addr().as_mut_ptr();
        // Map the address
        p3[i3].set_addr(
            frame.start_address(),
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        );
        true
    }

    /// # Sets P2->P1
    /// Tries to map a new table frame without allocating or using up frames.
    /// Returns true on success, and false if any entries were missing.
    /// This function does not flush TLB.
    /// Overwrites any existing mappings.
    #[must_use]
    unsafe fn try_direct_map_to_1(&self, page: Page, frame: PhysFrame) -> bool {
        let i2 = page.p2_index();
        let i3 = page.p3_index();
        let i4 = page.p4_index();

        // Resolve the P1 table
        let p4: &mut PageTable = &mut *self.p4_addr.as_mut_ptr();
        try_bool!(!p4[i4].is_unused());
        let p3: &mut PageTable = &mut *p4[i4].addr().as_mut_ptr();
        try_bool!(!p3[i3].is_unused());
        let p2: &mut PageTable = &mut *p3[i3].addr().as_mut_ptr();
        // Map the address
        p2[i2].set_addr(
            frame.start_address(),
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        );
        true
    }

    /// # Sets P1->P0
    /// Tries to map a new table frame without allocating or using up frames.
    /// Returns true on success, and false if any entries were missing.
    /// This function does not flush TLB.
    /// Overwrites any existing mappings.
    #[must_use]
    unsafe fn try_direct_map_to_0(&self, page: Page, frame: PhysFrame) -> bool {
        let i1 = page.p1_index();
        let i2 = page.p2_index();
        let i3 = page.p3_index();
        let i4 = page.p4_index();

        // Resolve the P1 table
        let p4: &mut PageTable = &mut *self.p4_addr.as_mut_ptr();
        try_bool!(!p4[i4].is_unused());
        let p3: &mut PageTable = &mut *p4[i4].addr().as_mut_ptr();
        try_bool!(!p3[i3].is_unused());
        let p2: &mut PageTable = &mut *p3[i3].addr().as_mut_ptr();
        try_bool!(!p2[i2].is_unused());
        let p1: &mut PageTable = &mut *p2[i2].addr().as_mut_ptr();
        // Map the address
        p1[i1].set_addr(
            frame.start_address(),
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        );
        true
    }

    pub unsafe fn map_to<A>(
        &mut self,
        page: Page,
        frame: PhysFrame,
        flags: Flags,
        allocator: &mut A,
    ) -> MapperFlush<pg::Size4KiB>
    where
        A: FrameAllocator<pg::Size4KiB>,
    {
        let pt_flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE;

        let i1 = page.p1_index();
        let i2 = page.p2_index();
        let i3 = page.p3_index();
        let i4 = page.p4_index();

        // Resolve the P1 table, filling in possibly missing higher-level tables
        let mut used_tables: usize = 0;

        let p4: &mut PageTable = &mut *self.p4_addr.as_mut_ptr();

        if p4[i4].is_unused() {
            // P3 table missing
            panic!("P3 MISSING"); // TODO: Over 512GiB virtual address required?
            used_tables = used_tables.max(3);
        }
        let p3: &mut PageTable = &mut *p4[i4].addr().as_mut_ptr();

        let p2: &mut PageTable = if p3[i3].is_unused() {
            // P2 table missing
            used_tables = used_tables.max(2);
            let frame = self.next_frame_for_level[1];
            p3[i3].set_addr(frame.start_address(), pt_flags);
            let mut table: &mut PageTable = unsafe { &mut *frame.start_address().as_mut_ptr() };
            table.zero();
            table
        } else {
            &mut *p3[i3].addr().as_mut_ptr()
        };

        let p1: &mut PageTable = if p2[i2].is_unused() {
            // P1 table missing
            used_tables = used_tables.max(1);
            let frame = self.next_frame_for_level[0];
            p2[i2].set_addr(frame.start_address(), pt_flags);
            let mut table: &mut PageTable = unsafe { &mut *frame.start_address().as_mut_ptr() };
            table.zero();
            table
        } else {
            &mut *p2[i2].addr().as_mut_ptr()
        };

        // Map the address
        p1[i1].set_addr(frame.start_address(), flags);

        // Allocate and map new `next_frame_for_level` frames as required
        for i in 0..used_tables {
            let frame = self.allocate_table(allocator);
            self.next_frame_for_level[i] = frame;
            // TODO: Flushing?
        }

        MapperFlush::new(page)
    }

    pub unsafe fn identity_map<A>(
        &mut self,
        frame: PhysFrame,
        flags: Flags,
        allocator: &mut A,
    ) -> MapperFlush<pg::Size4KiB>
    where
        A: FrameAllocator<pg::Size4KiB>,
    {
        let page =
            Page::from_start_address(VirtAddr::new_unchecked(frame.start_address().as_u64()))
                .expect("Invalid physical address: no corresponding virtual address");
        self.map_to(page, frame, flags, allocator)
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
