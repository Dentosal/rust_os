#![feature(allocator_api)]
#![feature(nonnull_slice_from_raw_parts)]
#![feature(slice_ptr_get)]
#![feature(const_fn)]
#![feature(integer_atomics)]
#![forbid(private_in_public)]
#![forbid(bare_trait_objects)]
#![deny(unused_assignments)]
#![no_std]

extern crate spin;

use core::alloc::{AllocError, AllocRef, GlobalAlloc, Layout};
use core::ptr::NonNull;

pub const HEAP_START: u64 = 0x4000_0000; // At 1 GiB
pub const HEAP_SIZE: u64 = 0x640_0000; // 100 MiB heap

use spin::Mutex;

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

use core::sync::atomic::{AtomicU64, Ordering};

/// A simple allocator that allocates memory linearly and ignores freed memory.
#[derive(Debug)]
pub struct BumpAllocator {
    heap_start: u64,
    heap_end: u64,
    next: AtomicU64,
}

impl BumpAllocator {
    pub const fn new(heap_start: u64, heap_end: u64) -> Self {
        Self {
            heap_start,
            heap_end,
            next: AtomicU64::new(heap_start),
        }
    }
}

unsafe impl<'a> AllocRef for &'a BumpAllocator {
    fn alloc(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        assert!(layout.size() > 0);
        loop {
            // load current state of the `next` field
            let current_next = self.next.load(Ordering::SeqCst);
            let alloc_start = align_up(current_next, layout.align() as u64);
            let alloc_end = alloc_start.saturating_add(layout.size() as u64);

            if alloc_end < self.heap_end {
                // update the `next` pointer if it still has the value `current_next`
                let next_now =
                    self.next
                        .compare_and_swap(current_next, alloc_end, Ordering::SeqCst);
                if next_now == current_next {
                    // next address was successfully updated, allocation succeeded
                    return Ok(NonNull::slice_from_raw_parts(
                        unsafe { NonNull::new_unchecked(alloc_start as *mut _) },
                        (alloc_end - alloc_start) as usize,
                    ));
                }
            } else {
                return Err(AllocError);
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // do nothing, leak memory
    }
}

pub struct GlobAlloc {
    alloc: Mutex<BumpAllocator>,
}
impl GlobAlloc {
    pub const fn new(alloc: BumpAllocator) -> Self {
        Self {
            alloc: Mutex::new(alloc),
        }
    }
}
unsafe impl GlobalAlloc for GlobAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let alloc = self.alloc.lock();
        (&*alloc)
            .alloc(layout)
            .expect("Could not allocate")
            .as_mut_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let alloc = self.alloc.lock();
        (&*alloc).dealloc(
            NonNull::new(ptr as *mut _).expect("Cannot deallocate null pointer"),
            layout,
        );
    }
}
