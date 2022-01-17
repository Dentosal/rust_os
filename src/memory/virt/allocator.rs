//! A first-fit virtual memory allocator

use alloc::vec::Vec;
use spin::Mutex;
use x86_64::VirtAddr;

use super::super::prelude::*;

const START_ADDR: VirtAddr = unsafe { VirtAddr::new_unsafe(0x100_000_000) };
const END_ADDR: VirtAddr = unsafe { VirtAddr::new_unsafe(0x200_000_000) };
const SIZE: u64 = END_ADDR.as_u64() - START_ADDR.as_u64();

lazy_static::lazy_static! {
    static ref FREE_LIST: Mutex<Vec<Block>> = Mutex::new(vec![Block {
                start: START_ADDR,
                end: END_ADDR,
            }]);
}

/// Allocate contiguous virtual address block
pub(super) fn allocate(size_pages: u64) -> VirtAddr {
    let mut free_blocks = FREE_LIST.lock();

    let mut i = 0;
    while i < free_blocks.len() {
        let block_size = free_blocks[i].size_pages();
        if block_size >= size_pages {
            return if block_size == size_pages {
                free_blocks.remove(i).start
            } else {
                let start = free_blocks[i].start;
                free_blocks[i].start += size_pages * PAGE_SIZE_BYTES;
                start
            };
        }
        i += 1;
    }
    panic!("Out of virtual memory");
}

pub(super) fn free(start: VirtAddr, size_pages: u64) {
    let mut free_blocks = FREE_LIST.lock();

    let index = match free_blocks.binary_search_by(|block| block.start.cmp(&start)) {
        Ok(_) => {
            panic!("VirtAlloc: Double free: {:?} ({} pages)", start, size_pages);
        },
        Err(i) => i,
    };

    // TODO: check for overlapping regions and report errors

    let end: VirtAddr = start + size_pages * PAGE_SIZE_BYTES;

    if index > 0 && free_blocks[index - 1].end == start {
        free_blocks[index - 1].end += size_pages * PAGE_SIZE_BYTES;
    } else if index < free_blocks.len() && free_blocks[index].start == end {
        free_blocks[index].start = start;
    } else {
        free_blocks.insert(index, Block { start, end });
    }
}

/// Range start..end, never empty
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Block {
    start: VirtAddr,
    end: VirtAddr,
}
impl Block {
    #[inline]
    pub fn size_bytes(&self) -> u64 {
        self.end.as_u64() - self.start.as_u64()
    }

    #[inline]
    pub fn size_pages(&self) -> u64 {
        self.size_bytes() / PAGE_SIZE_BYTES
    }

    #[inline]
    pub fn page_starts(&self) -> impl Iterator<Item = VirtAddr> {
        (self.start.as_u64()..self.end.as_u64())
            .step_by(PAGE_SIZE_BYTES as usize)
            .map(VirtAddr::new)
    }
}
