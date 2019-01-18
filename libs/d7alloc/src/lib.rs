#![feature(allocator_api)]
#![feature(const_fn)]

#![deny(warnings)]

#![no_std]

extern crate spin;

use core::ptr::NonNull;
use core::alloc::{GlobalAlloc, Alloc, AllocErr, Layout};

pub const HEAP_START: usize = 0x40000000; // At 1 GiB
pub const HEAP_SIZE: usize = 100 * 0x400;

use spin::Mutex;

/// Align downwards. Returns the greatest x with alignment `align`
/// so that x <= addr. The alignment must be a power of 2.
pub fn align_down(addr: usize, align: usize) -> usize {
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
pub fn align_up(addr: usize, align: usize) -> usize {
    align_down(addr + align - 1, align)
}

use core::sync::atomic::{AtomicUsize, Ordering};

/// A simple allocator that allocates memory linearly and ignores freed memory.
#[derive(Debug)]
pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: AtomicUsize,
}

impl BumpAllocator {
    pub const fn new(heap_start: usize, heap_end: usize) -> Self {
        Self { heap_start, heap_end, next: AtomicUsize::new(heap_start) }
    }
}

unsafe impl<'a> Alloc for &'a BumpAllocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<NonNull<u8>, AllocErr> {
        loop {
            // load current state of the `next` field
            let current_next = self.next.load(Ordering::Relaxed);
            let alloc_start = align_up(current_next, layout.align());
            let alloc_end = alloc_start.saturating_add(layout.size());

            if alloc_end <= self.heap_end {
                // update the `next` pointer if it still has the value `current_next`
                let next_now = self.next.compare_and_swap(current_next, alloc_end,
                    Ordering::Relaxed);
                if next_now == current_next {
                    // next address was successfully updated, allocation succeeded
                    return Ok(NonNull::new(alloc_start as *mut _).unwrap());
                }
            } else {
                return Err(AllocErr)
            }
        }
    }

    unsafe fn dealloc(&mut self, _ptr: NonNull<u8>, _layout: Layout) {
        // do nothing, leak memory
    }
}


pub struct GlobAlloc {
    alloc: Mutex<BumpAllocator>
}
impl GlobAlloc {
    pub const fn new(alloc: BumpAllocator) -> Self {
        Self {
            alloc: Mutex::new(alloc)
        }
    }
}
unsafe impl GlobalAlloc for GlobAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let alloc = self.alloc.lock();
        (&*alloc).alloc(layout).expect("Could not allocate").as_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let alloc = self.alloc.lock();
        (&*alloc).dealloc(NonNull::new(ptr as *mut _).expect("Cannot deallocate null pointer"), layout);
    }
}
