//! Data structures for working with multiple processors

use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::convert::TryFrom;
use core::sync::atomic::{AtomicU8, Ordering};
use spin::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{cpu_count, current_processor_id};

#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
enum InitState {
    Uninitialized,
    InProgress,
    Ready,
}

/// Stores data so that each CPU accesses their own version of it
pub struct PerCpu<T: Sized> {
    /// Invariant: Vec has fixed size after first allocation
    cell: UnsafeCell<Vec<Option<RwLock<T>>>>,
    /// Initialization status, as per `InitState`
    init: AtomicU8,
}
unsafe impl<T> Sync for PerCpu<T> {}
impl<T> PerCpu<T> {
    pub const fn new() -> Self {
        Self {
            cell: UnsafeCell::new(Vec::new()),
            init: AtomicU8::new(InitState::Uninitialized as u8),
        }
    }

    fn initialization_state(&self) -> InitState {
        InitState::try_from(self.init.load(Ordering::SeqCst)).unwrap()
    }

    fn ensure_initialized(&self) {
        loop {
            match self.init.compare_exchange_weak(
                InitState::Uninitialized as u8,
                InitState::InProgress as u8,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    // Now InProgress
                    let cell = self.cell.get();
                    // Safety: Locked with atomic self.init
                    unsafe {
                        let cc = cpu_count();
                        (*cell).reserve_exact(cc as usize);
                        for _ in 0..cc {
                            (*cell).push(None);
                        }
                    }
                    // Mark ready
                    self.init.store(InitState::Ready as u8, Ordering::SeqCst);
                    break;
                },
                Err(v) => {
                    log::info!("SPIN!");
                    match InitState::try_from(v).unwrap() {
                        // Spurious fail, allowed by the weak variant
                        InitState::Uninitialized => continue,
                        // In progress by some other thread
                        InitState::InProgress => continue,
                        // Already complete
                        InitState::Ready => break,
                    }
                },
            }
        }
    }

    pub fn set(&self, value: T) {
        self.ensure_initialized();

        let cell = self.cell.get();
        // Safety: Ensured
        unsafe {
            let c = (*cell)
                .get_mut(current_processor_id().0 as usize)
                .expect("CPU id index");

            *c = Some(RwLock::new(value));
        }
    }

    pub fn get(&self) -> Option<&RwLock<T>> {
        self.ensure_initialized();

        let cell = self.cell.get();
        // Safety: Ensured
        unsafe {
            (*cell)
                .get(current_processor_id().0 as usize)
                .expect("CPU id index")
                .as_ref()
        }
    }
}
