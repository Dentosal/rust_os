use core::alloc::{AllocError, AllocRef, GlobalAlloc, Layout};
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

/// A wrapper around spin::Mutex to permit trait implementations.
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

unsafe impl<'a> AllocRef for Locked<BlockAllocator> {
    fn alloc(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let mut ba = self.lock();

        // Calculate resulting pointer and required bytes
        let start_addr = align_up(
            PROCESS_DYNAMIC_MEMORY.as_u64() + ba.used_bytes,
            layout.align() as u64,
        );
        let required_size = layout.size() + layout.align();
        let required_size_u64 = required_size as u64;

        // Allocate more if required
        if ba.available_capacity_bytes() < required_size_u64 {
            let more = required_size_u64 - ba.available_capacity_bytes();
            let required_bytes = ba.capacity_bytes + more;
            ba.capacity_bytes = unsafe { mem_set_size(required_bytes).map_err(|_| AllocError)? };
            debug_assert!(required_size_u64 <= ba.available_capacity_bytes());
        }

        // Update used byte count and return
        ba.used_bytes += required_size_u64;

        Ok(NonNull::slice_from_raw_parts(
            unsafe { NonNull::new_unchecked(start_addr as *mut _) },
            required_size as usize,
        ))
    }

    unsafe fn dealloc(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // do nothing, leak memory
    }
}

pub struct GlobAlloc {
    alloc: Locked<BlockAllocator>,
}
impl GlobAlloc {
    pub const fn new(alloc: BlockAllocator) -> Self {
        Self {
            alloc: Locked::new(alloc),
        }
    }
}
unsafe impl GlobalAlloc for GlobAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc
            .alloc(layout)
            .expect("Could not allocate")
            .as_mut_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.alloc.dealloc(
            NonNull::new(ptr as *mut _).expect("Cannot deallocate null pointer"),
            layout,
        );
    }
}
