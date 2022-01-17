//! https://os.phil-opp.com/paging-implementation/#map-the-complete-physical-memory
//! https://wiki.osdev.org/Page_Tables
//!
//! Uses only 2MiB huge pages.

use x86_64::structures::paging as pg;
use x86_64::structures::paging::page_table::{PageTable, PageTableFlags as Flags};
use x86_64::PhysAddr;

use crate::util::elf_parser::{ELFData, ELFPermissionFlags};

use super::super::prelude::*;
use super::set_active_table;

macro_rules! try_bool {
    ($expr:expr) => {{
        if !$expr {
            return false;
        }
    }};
}

/// Maximum number of page tables
const MAX_PAGE_TABLES: u64 = HUGE_PAGE_SIZE / 0x1000;

macro_rules! get_page {
    ($base:expr, $index:literal) => {{ Page::from_start_address($base + 0x1000u64 * $index).unwrap() }};
    ($base:expr) => {{ Page::from_start_address($base).unwrap() }};
}

macro_rules! frame_addr {
    ($base:expr, $index:expr) => {{ $base + 0x1000u64 * $index }};
}

macro_rules! frame {
    ($base:expr, $index:expr) => {{ PhysFrame::from_start_address(frame_addr!($base, $index)).unwrap() }};
}

macro_rules! opt_addr {
    ($entry:expr) => {
        if $entry.is_unused() {
            None
        } else {
            Some($entry.addr())
        }
    };
}

macro_rules! pt_flags {
    (4) => {{ Flags::PRESENT | Flags::WRITABLE }};
    (3) => {{ Flags::PRESENT | Flags::WRITABLE }};
    (2) => {{ Flags::PRESENT | Flags::WRITABLE | Flags::HUGE_PAGE }};
}

/// # Paging manager
/// Uses one huge page for page tables themselves, allowing 0x200 tables,
/// totalling to 0x1fe * 0x200 * 0x200_000 = 0x7_f80_000_000 = 510 GiB of ram
///
/// NOTE: When/if expanding page map to have multiple pages, the current
/// implmentation requires adjacent pages in memory.
#[derive(Debug, Clone)]
pub struct PageMap {
    /// Physical address of the page table, and by extension, the P4 table
    pub phys_addr: PhysAddr,
    /// Next table will be placed to `PAGE_TABLE_AREA + PAGE_SIZE * table_count`,
    /// where `PAGE_SIZE` is `0x1000`.
    table_count: u64,
}

impl PageMap {
    /// Used during initialization
    pub const DUMMY: Self = Self {
        phys_addr: PhysAddr::zero(),
        table_count: 0,
    };

    /// Initializes a new page table structure.
    ///
    /// Given addesses must be P2 aligned (2MiB huge page).
    /// This way, only one P2 is required to create all the entries.
    ///
    /// # Arguments
    ///
    /// * `curr_addr`: Virtual address of this table accessible with the current page tables
    /// * `phys_addr`: Physical address of this table
    /// * `virt_addr`: Virtual address of this table when it's in use
    ///
    /// # Safety
    ///
    /// This function must only be called once per page table
    #[must_use]
    pub unsafe fn init(curr_addr: VirtAddr, phys_addr: PhysAddr, virt_addr: VirtAddr) -> Self {
        // We need to create and map one table for each level
        let p4_table: &mut PageTable = unsafe { &mut *frame_addr!(curr_addr, 0).as_mut_ptr() };
        let p3_table: &mut PageTable = unsafe { &mut *frame_addr!(curr_addr, 1).as_mut_ptr() };
        let p2_table: &mut PageTable = unsafe { &mut *frame_addr!(curr_addr, 2).as_mut_ptr() };

        // Zero the tables
        p4_table.zero();
        p3_table.zero();
        p2_table.zero();

        // Map P4->P3 and P3->P2
        let page = get_page!(virt_addr, 0);
        p4_table[page.p4_index()].set_addr(frame_addr!(phys_addr, 1), pt_flags!(4));
        p3_table[page.p3_index()].set_addr(frame_addr!(phys_addr, 2), pt_flags!(3));

        // Use P2 to map page table area
        p2_table[page.p2_index()].set_addr(frame_addr!(phys_addr, 0), pt_flags!(2));

        Self {
            phys_addr,
            //virt_addr,
            table_count: 3,
        }
    }

    /// Return the physical address for a given virtual address, if any available.
    ///
    /// This function does not flush the TLB.
    ///
    /// # Arguments
    ///
    /// * `curr_addr`: Virtual address of this table accessible with the current page tables
    /// * `addr`: Virtual address to resolve
    pub unsafe fn translate(&self, curr_addr: VirtAddr, addr: VirtAddr) -> Option<PhysAddr> {
        let p4: &PageTable = &*curr_addr.as_ptr();

        let next_addr = opt_addr!(p4[addr.p4_index()])?;
        let offset = next_addr - self.p4_addr();
        let p3: &PageTable = &*(curr_addr + offset).as_ptr();

        let next_addr = opt_addr!(p3[addr.p3_index()])?;
        let offset = next_addr - self.p4_addr();
        let p2: &PageTable = &*(curr_addr + offset).as_ptr();

        let next_addr = opt_addr!(p2[addr.p2_index()])?;
        assert!(p2[addr.p2_index()].flags().contains(Flags::HUGE_PAGE));

        Some(next_addr)
    }

    #[inline(always)]
    pub fn p4_addr(&self) -> PhysAddr {
        self.phys_addr
    }

    /// Sets the current table as the active page table
    #[inline(always)]
    pub(super) unsafe fn activate(&self) {
        set_active_table(self.p4_addr());
    }

    /// Removes a mapping
    pub unsafe fn unmap(&mut self, curr_addr: VirtAddr, page: Page) -> MapperFlush<pg::Size2MiB> {
        let i4 = page.p4_index();
        let i3 = page.p3_index();
        let i2 = page.p2_index();

        let p4: &mut PageTable = &mut *curr_addr.as_mut_ptr();
        let offset: isize = (curr_addr.as_u64() as isize) - (self.p4_addr().as_u64() as isize);

        if p4[i4].is_unused() {
            panic!("Unmap nonexistent: P3 missing");
        }

        let p3: &mut PageTable =
            &mut *((p4[i4].addr().as_u64() as isize + offset) as *mut PageTable);

        if p3[i3].is_unused() {
            panic!("Unmap nonexistent: P2 missing");
        }
        let p2: &mut PageTable =
            &mut *((p3[i3].addr().as_u64() as isize + offset) as *mut PageTable);

        // Unmap the address
        p2[i2].set_unused();

        MapperFlush::new(page)
    }

    /// Maps a frame to a page using given flags
    ///
    /// `curr_addr` is the virtual address of this table in the current page tables
    pub unsafe fn map_to(
        &mut self, curr_addr: VirtAddr, page: Page, frame: PhysFrame, flags: Flags,
    ) -> MapperFlush<pg::Size2MiB> {
        let i4 = page.p4_index();
        let i3 = page.p3_index();
        let i2 = page.p2_index();

        // Tables under P4 need a translation when walking
        let offset: isize = (curr_addr.as_u64() as isize) - (self.p4_addr().as_u64() as isize);

        // Resolve the P2 table, filling in possibly missing higher-level tables,
        // and then map the actual address

        let p4: &mut PageTable = &mut *curr_addr.as_mut_ptr();

        let p3: &mut PageTable = if p4[i4].is_unused() {
            // P3 table missing
            let addr = frame_addr!(self.phys_addr, self.table_count);
            self.table_count += 1;
            p4[i4].set_addr(addr, pt_flags!(3));
            let table: &mut PageTable =
                unsafe { &mut *((addr.as_u64() as isize + offset) as *mut PageTable) };
            table.zero();
            table
        } else {
            &mut *((p4[i4].addr().as_u64() as isize + offset) as *mut PageTable)
        };

        let p2: &mut PageTable = if p3[i3].is_unused() {
            // P2 table missing
            let addr = frame_addr!(self.phys_addr, self.table_count);
            self.table_count += 1;
            p3[i3].set_addr(addr, pt_flags!(3));
            let table: &mut PageTable =
                unsafe { &mut *((addr.as_u64() as isize + offset) as *mut PageTable) };
            table.zero();
            table
        } else {
            &mut *((p3[i3].addr().as_u64() as isize + offset) as *mut PageTable)
        };

        // Map the address
        p2[i2].set_addr(frame.start_address(), flags | Flags::HUGE_PAGE);

        // log::trace!(
        //     "mapped {:?} to {:?} with {:?}",
        //     frame,
        //     page,
        //     flags | Flags::HUGE_PAGE
        // );

        MapperFlush::new(page)
    }

    pub unsafe fn identity_map(
        &mut self, curr_addr: VirtAddr, frame: PhysFrame, flags: Flags,
    ) -> MapperFlush<pg::Size2MiB> {
        let page = Page::from_start_address(VirtAddr::new_unsafe(frame.start_address().as_u64()))
            .expect("Invalid physical address: no corresponding virtual address");
        self.map_to(curr_addr, page, frame, flags)
    }

    /// Map other page table into this address space
    pub unsafe fn map_table(
        &mut self, curr_addr: VirtAddr, page: Page, other: &Self, writable: bool,
    ) -> MapperFlush<pg::Size2MiB> {
        let mut flags = Flags::PRESENT | Flags::NO_EXECUTE;
        if writable {
            flags |= Flags::WRITABLE;
        }

        self.map_to(
            curr_addr,
            page,
            PhysFrame::from_start_address(other.p4_addr()).unwrap(),
            flags,
        )
    }

    /// Maps elf executable based om program headers
    /// This function does not flush the TLB
    /// NOTE: This is only suitable for mapping the kernel itself
    pub unsafe fn identity_map_elf(&mut self, curr_addr: VirtAddr, elf_metadata: ELFData) {
        for ph in elf_metadata.ph_table.iter().filter_map(|x| *x) {
            if ph.loadable() {
                let start = PhysAddr::new(ph.virtual_address);
                let size = ph.size_in_memory;
                let mut flags = Flags::PRESENT;

                assert!(start.as_u64() % Page::SIZE == 0);
                assert!(size > 0);

                if !ph.has_flag(ELFPermissionFlags::EXECUTABLE) {
                    flags |= Flags::NO_EXECUTE;
                }
                if !ph.has_flag(ELFPermissionFlags::READABLE) {
                    panic!("Non-readable pages are not supported (yet)");
                }
                if ph.has_flag(ELFPermissionFlags::WRITABLE) {
                    flags |= Flags::WRITABLE;
                }

                // log::debug!("{:#x} :+ {:#x} [{:?}]", start, size, flags);

                let start_frame = PhysFrame::containing_address(start);
                let end_frame = PhysFrame::containing_address(start + (size - 1));
                for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
                    self.identity_map(curr_addr, frame, flags).ignore();
                }
            }
        }
    }
}

/// TLB flush information for a page
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
        // x86_64::instructions::tlb::flush(self.0.start_address());
        x86_64::instructions::tlb::flush_all();
    }

    /// Explicitly skip flushing the change
    pub fn ignore(self) {}
}
