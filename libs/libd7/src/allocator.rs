use core::alloc::{Alloc, AllocErr, GlobalAlloc, Layout};
use core::ptr::NonNull;

use spin::Mutex;

use super::syscall::mem_set_size;
use d7abi::PROCESS_DYNAMIC_MEMORY;

/// Align downwards. Returns the greatest x with alignment `align`
/// so that x <= addr. The alignment must be a power of 2.
pub fn align_down(addr: u64, align: u64) -> u64 {
    if align.is_power_of_two() {
        addr & !(align - 1)
    } else if align == 0 {
        addr
    } else {
        panic!("`align` must be a power of 2");
    }
}

/// Align upwards. Returns the smallest x with alignment `align`
/// so that x >= addr. The alignment must be a power of 2.
pub fn align_up(addr: u64, align: u64) -> u64 {
    align_down(addr + align - 1, align)
}

/// A simple allocator that allocates memory linearly and ignores freed memory.
/// Uses atomics, but is not thread-safe! Must be placed behind a Mutex.
#[derive(Debug)]
pub struct BlockAllocator {
    /// Used size
    used_bytes: u64,
    /// Alllocated capacity
    capacity_bytes: u64,
}

impl BlockAllocator {
    pub const fn new() -> Self {
        Self {
            used_bytes: 0,
            capacity_bytes: 0,
        }
    }

    pub fn available_capacity_bytes(&self) -> u64 {
        self.capacity_bytes - self.used_bytes
    }
}

unsafe impl<'a> Alloc for BlockAllocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<NonNull<u8>, AllocErr> {
        // Calculate resulting pointer and required bytes
        let start_addr = align_up(
            PROCESS_DYNAMIC_MEMORY.as_u64() + self.used_bytes,
            layout.align() as u64,
        );
        let required_size = (layout.size() + layout.align()) as u64;

        // Allocate more if required
        if self.available_capacity_bytes() < required_size {
            let more = required_size - self.available_capacity_bytes();
            let required_bytes = self.capacity_bytes + more;
            self.capacity_bytes = mem_set_size(required_bytes).map_err(|_| AllocErr)?;
            debug_assert!(required_size <= self.available_capacity_bytes());
        }

        // Update used byte count and return
        self.used_bytes += required_size;
        Ok(NonNull::new_unchecked(start_addr as *mut _))
    }

    unsafe fn dealloc(&mut self, _ptr: NonNull<u8>, _layout: Layout) {
        // do nothing, leak memory
    }
}

pub struct GlobAlloc {
    alloc: Mutex<BlockAllocator>,
}
impl GlobAlloc {
    pub const fn new(alloc: BlockAllocator) -> Self {
        Self {
            alloc: Mutex::new(alloc),
        }
    }
}
unsafe impl GlobalAlloc for GlobAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut alloc = self.alloc.lock();
        alloc.alloc(layout).expect("Could not allocate").as_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut alloc = self.alloc.lock();
        alloc.dealloc(
            NonNull::new(ptr as *mut _).expect("Cannot deallocate null pointer"),
            layout,
        );
    }
}
