//! Kernel heap

use core::alloc::Layout;
use x86_64::{PhysAddr, VirtAddr};

use super::{area::PhysMemoryRange, phys_to_virt};

mod allocator;
mod list;

pub use self::allocator::*;
pub use self::list::AllocationSet;

/// A freeable allocation
#[derive(Debug)]
pub struct Allocation {
    pub(super) start: PhysAddr,
    pub(super) layout: Layout,
}

impl Drop for Allocation {
    fn drop(&mut self) {
        log::trace!("Drop-deallocate {:?}", self);

        // Safety: will only be called once
        unsafe {
            _deallocate(self);
        }

        let zero = PhysAddr::zero();
        let old = core::mem::replace(&mut self.start, zero);
        if old == zero {
            panic!("Douple-drop!");
        }
    }
}

impl Allocation {
    /// Leaks this allocation, making it impossible to deallocate
    pub fn leak(self) -> PhysMemoryRange {
        let result = PhysMemoryRange::range(self.start..self.start + self.layout.size());
        core::mem::forget(self);
        result
    }

    pub unsafe fn phys_start(&self) -> PhysAddr {
        self.start
    }

    pub unsafe fn mapped_start(&self) -> VirtAddr {
        unsafe { phys_to_virt(self.start) }
    }

    pub unsafe fn from_mapped(start: *mut u8, layout: Layout) -> Self {
        Self {
            start: PhysAddr::new_unchecked(undo_offset_ptr(start) as u64),
            layout,
        }
    }

    pub fn size(&self) -> usize {
        self.layout.size()
    }

    pub fn read(&self) -> &[u8] {
        // Safety: tied to the lifetime of self
        unsafe {
            core::slice::from_raw_parts(
                self.mapped_start().as_ptr_unchecked(),
                self.layout.size() as usize,
            )
        }
    }

    pub fn write(&mut self) -> &mut [u8] {
        // Safety: requires exclusive access of self, tied lifetime
        unsafe {
            core::slice::from_raw_parts_mut(
                self.mapped_start().as_mut_ptr_unchecked(),
                self.layout.size() as usize,
            )
        }
    }
}
