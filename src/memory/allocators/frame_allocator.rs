//! Physical memory allocator
//! Currently just leaks freed frames
//! TODO: A proper allocator, including free

use x86_64::structures::paging as pg;
use x86_64::PhysAddr;

use super::super::area::PhysMemoryRange;
use super::super::map::MAX_OK_ENTRIES;
use super::super::prelude::*;

pub struct Allocator {
    /// Physical memory map, i.e. usable ram regions
    areas: [Option<PhysMemoryRange>; MAX_OK_ENTRIES],
    /// Number of all available frames
    total_frames: usize,
    /// Next free frame
    next_free: usize,
    // /// How many frames are reserved for bookkeeping
    // bookkeep_frames: usize,
}

impl Allocator {
    /// Unsafe, as the caller is responsibe that this is not intialized multiple times
    pub unsafe fn new(areas: [Option<PhysMemoryRange>; MAX_OK_ENTRIES]) -> Self {
        Self {
            areas,
            total_frames: areas
                .iter()
                .filter_map(|opt| opt.map(|a| a.size_pages() as usize))
                .sum(),
            next_free: 0,
            // bookkeep_frames: 0,
        }
    }

    /// Maps internal contiguous index to page start address
    /// Panics if out of bounds
    fn to_page_addr(&self, mut index: usize) -> PhysAddr {
        for area in self.areas.iter().filter_map(|opt| *opt) {
            let size = area.size_pages() as usize;
            if index < size {
                return area.start() + Page::SIZE * (index as u64);
            } else {
                index -= size;
            }
        }
        panic!("Allocator index out of bounds");
    }

    fn is_free(&self, index: usize) -> bool {
        self.next_free <= index
    }
}
unsafe impl pg::FrameAllocator<PageSizeType> for Allocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        if self.next_free == self.total_frames {
            log::error!("No more physical memory available");
            None
        } else {
            let frame = PhysFrame::from_start_address(self.to_page_addr(self.next_free))
                .expect("to_page_addr generated misaligned address");
            self.next_free += 1;
            Some(frame)
        }
    }

    // fn deallocate_frame(&mut self, frame: PhysFrame) {
    //     if self.is_free(frame.index) {
    //         panic!("deallocate_frame: Page {} is already free.", frame.index);
    //     }

    //     // Just leak it
    // }
}
