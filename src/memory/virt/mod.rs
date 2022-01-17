use x86_64::VirtAddr;

use super::prelude::*;

mod allocator;

/// Allocate a contiguous virtual address block
pub fn allocate(size_pages: usize) -> Allocation {
    let start = allocator::allocate(size_pages as u64);
    Allocation {
        start,
        end: start + size_pages * (PAGE_SIZE_BYTES as usize),
    }
}

/// An owned virtual address space allocation, freed on drop
#[derive(Debug, PartialEq, Eq)]
pub struct Allocation {
    pub start: VirtAddr,
    pub end: VirtAddr,
}

impl Drop for Allocation {
    fn drop(&mut self) {
        allocator::free(self.start, self.size_pages())
    }
}

impl Allocation {
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
