use x86_64::VirtAddr;

use alloc::vec::Vec;

use super::prelude::PAGE_SIZE_BYTES;

const START_ADDR: VirtAddr = unsafe { VirtAddr::new_unchecked_raw(0x100_000_000) };
const END_ADDR: VirtAddr = unsafe { VirtAddr::new_unchecked_raw(0x200_000_000) };
const SIZE: u64 = END_ADDR.as_u64() - START_ADDR.as_u64();

/// A first-fit virtual memory allocator
/// TODO: Just allocate frames and don't consume heap space
#[derive(Debug)]
pub struct VirtualAllocator {
    free_blocks: Vec<Area>,
}
impl VirtualAllocator {
    /// Must be called only once
    pub fn new() -> Self {
        Self {
            free_blocks: vec![Area {
                start: START_ADDR,
                end: END_ADDR,
            }],
        }
    }

    /// Allocate contiguous virtual address block
    pub fn allocate(&mut self, size_pages: u64) -> VirtAddr {
        let mut i = 0;
        while i < self.free_blocks.len() {
            let block_size = self.free_blocks[i].size_pages();
            if block_size >= size_pages {
                return if block_size == size_pages {
                    self.free_blocks.remove(i).start
                } else {
                    let start = self.free_blocks[i].start;
                    self.free_blocks[i].start += size_pages * PAGE_SIZE_BYTES;
                    start
                };
            }
            i += 1;
        }
        panic!("Out of virtual memory");
    }

    pub fn free(&mut self, start: VirtAddr, size_pages: u64) {
        unimplemented!("TODO: Free virtual memory")
    }
}

/// Range start..end
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Area {
    pub start: VirtAddr,
    pub end: VirtAddr,
}
impl Area {
    pub fn new_pages(start: VirtAddr, size_pages: u64) -> Self {
        Self {
            start,
            end: start + size_pages * PAGE_SIZE_BYTES,
        }
    }

    pub fn size_pages(&self) -> u64 {
        self.end.as_u64() / self.start.as_u64()
    }
}
