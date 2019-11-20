use x86_64::VirtAddr;

use alloc::vec::Vec;

use super::super::prelude::*;

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
        let index = match self
            .free_blocks
            .binary_search_by(|block| block.start.cmp(&start))
        {
            Ok(_) => {
                panic!("VirtAlloc: Double free: {:?} ({} pages)", start, size_pages);
            },
            Err(i) => i,
        };

        // TODO: check for overlapping regions and report errors

        let end: VirtAddr = start + size_pages * PAGE_SIZE_BYTES;

        if index > 0 && self.free_blocks[index - 1].end == start {
            self.free_blocks[index - 1].end += size_pages * PAGE_SIZE_BYTES;
        } else if index < self.free_blocks.len() && self.free_blocks[index].start == end {
            self.free_blocks[index].start = start;
        } else {
            self.free_blocks.insert(index, Area { start, end });
        }
    }
}

/// Range start..end, never empty
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub start: VirtAddr,
    pub end: VirtAddr,
}
impl Area {
    pub fn new_pages(start: VirtAddr, size_pages: u64) -> Self {
        debug_assert_ne!(size_pages, 0);
        Self {
            start,
            end: start + size_pages * PAGE_SIZE_BYTES,
        }
    }

    pub fn new_containing_block(start: VirtAddr, size_bytes: u64) -> Self {
        debug_assert_ne!(size_bytes, 0);
        Self {
            start: page_align(start, false),
            end: page_align(start + size_bytes, true),
        }
    }

    #[inline]
    pub fn size_bytes(&self) -> u64 {
        self.end.as_u64() - self.start.as_u64()
    }

    #[inline]
    pub fn size_pages(&self) -> u64 {
        self.size_bytes() / PAGE_SIZE_BYTES
    }

    #[inline]
    pub fn pages(&self) -> impl Iterator<Item = VirtAddr> {
        (self.start.as_u64()..self.end.as_u64())
            .step_by(PAGE_SIZE_BYTES as usize)
            .map(VirtAddr::new)
        // let result: Vec<VirtAddr> = Vec::new();
        // let mut cursor = self.start;
        // while cursor < self.end {
        //     result.push(cursor);
        //     cursor += PAGE_SIZE_BYTES;
        // }
        // result
    }
}
