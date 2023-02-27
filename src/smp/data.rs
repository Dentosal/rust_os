//! Data structures for working with multiple processors

use alloc::vec::Vec;
use spin::Once;

use super::{available_cpu_count, current_processor_id, ProcessorId};

/// Stores data so that each CPU accesses their own version of it.
pub struct PerCpu<T: Sized> {
    /// The vector fixed size after first allocation.
    slots: Once<Vec<T>>,
}
impl<T: Default> PerCpu<T> {
    pub const fn new() -> Self {
        Self { slots: Once::new() }
    }

    pub fn for_cpu(&self, processor_id: ProcessorId) -> &T {
        let slots = self.slots.call_once(|| {
            let limit = available_cpu_count();
            let mut vec = Vec::with_capacity(limit);
            for _ in 0..limit {
                vec.push(T::default());
            }
            vec
        });

        slots
            .get(processor_id.0 as usize)
            .expect("ProcessorId > available_cpu_count()")
    }

    pub fn current_cpu(&self) -> &T {
        self.for_cpu(current_processor_id())
    }
}
