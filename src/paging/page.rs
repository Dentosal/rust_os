use mem_map::MEM_PAGE_SIZE_BYTES;
use mem_map::{Frame,FrameAllocator};

use super::ENTRY_COUNT;
use super::VirtualAddress;
use super::mapper::Mapper;
use super::table::*;
use super::page_table::ActivePageTable;
use super::mapper;
use super::entry::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page {
   pub index: usize,
}

impl Page {
    pub fn containing_address(address: VirtualAddress) -> Page {
        assert!(address < 0x0000_8000_0000_0000 || address >= 0xffff_8000_0000_0000, "invalid address: 0x{:x}", address);
        Page { index: address / MEM_PAGE_SIZE_BYTES }
    }

    pub fn start_address(&self) -> usize {
        self.index * MEM_PAGE_SIZE_BYTES
    }

    pub fn p4_index(&self) -> usize {
        (self.index >> 27) & 0o777
    }
    pub fn p3_index(&self) -> usize {
        (self.index >> 18) & 0o777
    }
    pub fn p2_index(&self) -> usize {
        (self.index >> 9) & 0o777
    }
    pub fn p1_index(&self) -> usize {
        (self.index >> 0) & 0o777
    }

    pub fn translate(&self) -> Option<Frame> {
        let p3 = unsafe { &*mapper::P4 }.next_table(self.p4_index());

        let huge_page = || {
            p3.and_then(|p3| {
                let p3_entry = &p3[self.p3_index()];
                // 1GiB self?
                if let Some(start_frame) = p3_entry.pointed_frame() {
                    if p3_entry.flags().contains(HUGE_PAGE) {
                        // address must be 1GiB aligned
                        assert!(start_frame.index % (ENTRY_COUNT * ENTRY_COUNT) == 0);
                        return Some(Frame {
                            index: start_frame.index + self.p2_index() * ENTRY_COUNT + self.p1_index()
                        });
                    }
                }
                if let Some(p2) = p3.next_table(self.p3_index()) {
                    let p2_entry = &p2[self.p2_index()];
                    // 2MiB self?
                    if let Some(start_frame) = p2_entry.pointed_frame() {
                        if p2_entry.flags().contains(HUGE_PAGE) {
                            // address must be 2MiB aligned
                            assert!(start_frame.index % ENTRY_COUNT == 0);
                            return Some(Frame { index: start_frame.index + self.p1_index() });
                        }
                    }
                }
                None
            })
        };

        p3.and_then(|p3| p3.next_table(self.p3_index()))
          .and_then(|p2| p2.next_table(self.p2_index()))
          .and_then(|p1| p1[self.p1_index()].pointed_frame())
          .or_else(huge_page)
    }
    pub fn range_inclusive(start: Page, end: Page) -> PageIter {
        PageIter {
            start: start,
            end: end,
        }
    }
}

pub struct PageIter {
    start: Page,
    end: Page,
}

impl Iterator for PageIter {
    type Item = Page;

    fn next(&mut self) -> Option<Page> {
        if self.start <= self.end {
            let page = self.start;
            self.start.index += 1;
            Some(page)
        } else {
            None
        }
    }
}

struct TinyAllocator([Option<Frame>; 3]);
impl TinyAllocator {
    fn new<A>(allocator: &mut A) -> TinyAllocator where A: FrameAllocator
    {
        let mut f = || allocator.allocate_frame();
        let frames = [f(), f(), f()];
        TinyAllocator(frames)
    }
}
impl FrameAllocator for TinyAllocator {
    fn allocate_frame(&mut self) -> Option<Frame> {
        for frame_option in &mut self.0 {
            if frame_option.is_some() {
                return frame_option.take();
            }
        }
        None
    }

    fn deallocate_frame(&mut self, frame: Frame) {
        for frame_option in &mut self.0 {
            if frame_option.is_none() {
                *frame_option = Some(frame);
                return;
            }
        }
        panic!("Tiny allocator can hold only 3 frames.");
    }
}


pub struct TemporaryPage {
    page: Page,
    allocator: TinyAllocator,
}
impl TemporaryPage {
    pub fn new<A>(page: Page, allocator: &mut A) -> TemporaryPage where A: FrameAllocator {
        TemporaryPage {
            page: page,
            allocator: TinyAllocator::new(allocator),
        }
    }
    /// Maps the temporary page to the given frame in the active table.
    /// Returns the start address of the temporary page.
    pub fn map(&mut self, frame: Frame, active_table: &mut ActivePageTable) -> VirtualAddress {
        // XXX: is this kluge fix? tutorial doesn't seem to have the same problem
        // assert!(active_table.translate_page(self.page).is_none(), "temporary page is already mapped");
        assert!(active_table.translate_page(self.page.index).is_none(), "temporary page is already mapped");

        active_table.map_to(self.page, frame, WRITABLE, &mut self.allocator);
        self.page.start_address()
    }

    pub fn map_table_frame(&mut self, frame: Frame, active_table: &mut ActivePageTable) -> &mut Table<Level1> {
        unsafe { &mut *(self.map(frame, active_table) as *mut Table<Level1>) }
    }

    /// Unmaps the temporary page in the active table.
    pub fn unmap(&mut self, active_table: &mut Mapper) {
        active_table.unmap(self.page, &mut self.allocator)
    }
}
