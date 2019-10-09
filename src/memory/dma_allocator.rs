use spin::Mutex;

use super::prelude::*;
use x86_64::structures::paging::PageSize;

// Must be kept in sync with plan.md
pub const BASE: PhysAddr = unsafe { PhysAddr::new_unchecked(0x20000) };
pub const SIZE: u64 = 0x50000;

pub const ENTRY_COUNT: usize = (SIZE / Page::SIZE) as usize;
const BITMAP_SIZE: usize = (ENTRY_COUNT + 7) / 8;

pub struct Allocator {
    reserved: [u8; BITMAP_SIZE], // bitmaps, ceil(count)
}
impl Allocator {
    const fn new() -> Allocator {
        Allocator {
            reserved: [0; BITMAP_SIZE],
        }
    }

    fn is_free(&self, index: usize) -> bool {
        assert!(index < ENTRY_COUNT);
        self.reserved[index / 8] & (1 << (index % 8)) == 0
    }

    fn reserve(&mut self, index: usize) {
        assert!(index < ENTRY_COUNT);
        assert!(self.is_free(index));
        self.reserved[index / 8] |= (1 << (index % 8));
    }

    fn release(&mut self, index: usize) {
        assert!(index < ENTRY_COUNT);
        assert!(!self.is_free(index));
        self.reserved[index / 8] &= !(1 << (index % 8));
    }

    pub fn allocate_blocks(&mut self, count: usize) -> Option<*mut u8> {
        assert!(count < ENTRY_COUNT);

        'outer: for index in 0..(ENTRY_COUNT - count) {
            for i in 0..count {
                if !self.is_free(index + i) {
                    continue 'outer;
                }
            }
            for i in 0..count {
                self.reserve(index + i);
            }
            return Some((BASE + (index as u64) * Page::SIZE).as_u64() as *mut _);
        }

        None
    }
}

// Create static pointer mutex with spinlock to make the allocator thread-safe
pub static DMA_ALLOCATOR: Mutex<Allocator> = Mutex::new(Allocator::new());
