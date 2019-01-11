use core::ptr::Unique;

use mem_map::MEM_PAGE_SIZE_BYTES;
use mem_map::{Frame, FrameAllocator};

use super::{VirtualAddress, PhysicalAddress};
use super::page::Page;
use super::entry::*;
use super::table::{Table, Level4};
use super::tlb;

pub struct Mapper {
    p4: Unique<Table<Level4>>
}

impl Mapper {
    pub unsafe fn new() -> Mapper {
        Mapper { p4: Unique::new_unchecked(P4) }
    }

    pub fn map_to<A>(&mut self, page: Page, frame: Frame, flags: EntryFlags, allocator: &mut A) where A: FrameAllocator {
        let p4 = unsafe { &mut *P4 };
        let p3 = p4.next_table_create(page.p4_index(), allocator);
        let p2 = p3.next_table_create(page.p3_index(), allocator);
        let p1 = p2.next_table_create(page.p2_index(), allocator);

        assert!(p1[page.p1_index()].is_unused());
        p1[page.p1_index()].set(frame, flags | EntryFlags::PRESENT);
    }

    pub fn translate_page(&self, virtual_address: VirtualAddress) -> Option<PhysicalAddress> {
        let offset = virtual_address % MEM_PAGE_SIZE_BYTES;
        Page::containing_address(virtual_address).translate().map(|frame| frame.index * MEM_PAGE_SIZE_BYTES + offset)
    }

    pub fn map<A>(&mut self, page: Page, flags: EntryFlags, allocator: &mut A) where A: FrameAllocator {
        let frame = allocator.allocate_frame().expect("out of memory");
        // rprintln!("!m {:#x} {:#x}", frame.start_address() as u64, page.index);
        self.map_to(page, frame, flags, allocator);
    }

    pub fn identity_map<A>(&mut self, frame: Frame, flags: EntryFlags, allocator: &mut A) where A: FrameAllocator {
        let page = Page::containing_address(frame.start_address());
        // rprintln!("!i {:#x} {:#x}", frame.start_address() as u64, page.index);
        self.map_to(page, frame, flags, allocator);
    }

    pub fn unmap<A>(&mut self, page: Page, allocator: &mut A) where A: FrameAllocator {
        // FIXME: ?? http://os.phil-opp.com/modifying-page-tables.html#unmapping-pages
        assert!(self.translate_page(page.start_address()).is_some());

        let p1 = self.p4_mut()
                     .next_table_mut(page.p4_index())
                     .and_then(|p3| p3.next_table_mut(page.p3_index()))
                     .and_then(|p2| p2.next_table_mut(page.p2_index()))
                     .expect("mapping code does not support huge pages");

        let frame = p1[page.p1_index()].pointed_frame().unwrap();
        p1[page.p1_index()].set_unused();

        unsafe {
            tlb::flush(page.start_address());
        }

        // TODO free p(1,2,3) table if empty
        allocator.deallocate_frame(frame);
    }

    pub fn p4(&self) -> &Table<Level4> {
        unsafe { self.p4.as_ref() }
    }

    pub fn p4_mut(&mut self) -> &mut Table<Level4> {
        unsafe { self.p4.as_mut() }
    }
}

pub const P4: *mut Table<Level4> = 0xffffffff_fffff000 as *mut _;
// pub const P4: *mut Table<Level4> = 0x20000 as *mut _;
