//! Data structures for working with multiple processors

use alloc::vec::Vec;

use super::{cpu_count, current_processor_id};

/// Stores data so that each CPU accesses their own version of it
pub struct PerCpu<T: Sized> {
    /// Invariant: fixed size after first allocation
    values: Vec<Option<T>>,
}
impl<T> PerCpu<T> {
    pub const fn new() -> Self {
        Self { values: Vec::new() }
    }

    fn initialized(&self) -> bool {
        self.values.capacity() != 0
    }

    fn initialize(&mut self) {
        self.values.reserve_exact(cpu_count() as usize);
    }

    pub fn set(&mut self, value: T) {
        if !self.initialized() {
            self.initialize();
        }

        unsafe {
            let v = self
                .values
                .get_unchecked_mut(current_processor_id().0 as usize);
            *v = Some(value);
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.initialized() {
            unsafe {
                self.values
                    .get_unchecked(current_processor_id().0 as usize)
                    .as_ref()
            }
        } else {
            None
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.initialized() {
            unsafe {
                self.values
                    .get_unchecked_mut(current_processor_id().0 as usize)
                    .as_mut()
            }
        } else {
            None
        }
    }
}
