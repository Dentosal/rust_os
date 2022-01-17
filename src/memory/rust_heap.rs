//! Kernel heap for Rust's global_allocator

use alloc::alloc::{GlobalAlloc, Layout};
use core::{mem, ptr};
use spin::Mutex;

use allogator::BlockLLAllocator;

use super::{
    area::PhysMemoryRange,
    phys::{self, AllocationSet},
    PAGE_LAYOUT,
};

/// Smallest allocation. Smaller than this, will be rounded up.
const MIN_ALLOC: usize = mem::size_of::<*mut u8>();

/// If this is reached, a full physical buddy is allocated.
const MIN_BUDDY: usize = 0x10_0000; // Reduce this to around 1KiB when small pages are supported

struct SmallAlloc {
    set: phys::AllocationSet<BlockLLAllocator>,
}
impl SmallAlloc {
    unsafe fn allocate(&mut self, size: usize) -> *mut u8 {
        let size = size.next_power_of_two();

        // First, check if we have any free slots of this size
        let found = self.set.iterate_mut_first(|block_ll| {
            if block_ll.item_size() == size && !block_ll.is_full() {
                return Some(block_ll.allocate_one().unwrap().as_ptr());
            }
            None
        });

        if let Some(r) = found {
            return r;
        }

        // Otherwise, add a new BlockLL for this size
        let allocation = phys::allocate(PAGE_LAYOUT).expect("Failed to allocate");

        let ptr: *mut u8 = allocation.mapped_start().as_mut_ptr();
        let len = allocation.size();

        let backing = core::slice::from_raw_parts_mut(ptr, len);

        // Allocate our object
        let block_ll = BlockLLAllocator::new(backing, size);
        let result = block_ll.allocate_one().unwrap().as_ptr();

        self.set.push(block_ll);

        result
    }

    unsafe fn deallocate(&mut self, ptr: *mut u8, size: usize) {
        let ptr = ptr::NonNull::new(ptr).unwrap();

        self.set.iterate_mut_first(|block_ll| {
            if block_ll.item_size() == size && block_ll.contains(ptr) {
                block_ll.deallocate_one(ptr);
                // TODO: if the block is empty, should it be deallocated?
                Some(())
            } else {
                None
            }
        });
    }
}

pub struct GlobAlloc {
    inner: Mutex<SmallAlloc>,
}
impl GlobAlloc {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(SmallAlloc {
                set: AllocationSet::EMPTY,
            }),
        }
    }
}
unsafe impl GlobalAlloc for GlobAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let req = layout.size().max(layout.align()).max(MIN_ALLOC);
        if req >= MIN_BUDDY {
            phys::allocate(layout)
                .expect("Rust heap alloc failed")
                .mapped_start()
                .as_mut_ptr()
        } else {
            let mut inner = self.inner.lock();
            inner.allocate(req)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let req = layout.size().max(layout.align()).max(MIN_ALLOC);
        if req >= MIN_BUDDY {
            phys::deallocate(phys::Allocation::from_mapped(ptr, layout))
        } else {
            let mut inner = self.inner.lock();
            inner.deallocate(ptr, req)
        }
    }
}
