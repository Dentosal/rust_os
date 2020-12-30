//! Time is measured using TSC on the BSP core.
//! As the TSC might roll over in 10 years or so from boot,
//! it might be good to reset TSC to zero when it's near wraparound,
//! and then increment some global epoch variable.
//!
//! Other cores should very rarely need access to this time.
//! The BSP core is the only core that moves tasks out of the sleep queue,
//! so schduler times are stored in (future) TSC timestamps of the BSP.

use crate::driver::tsc;
use crate::smp::is_bsp;

/// Timestamp relative to the TSC of the BSP core.
/// All functions are requiring read access to the TSC
/// are only accessible on the BSP core and panic otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BSPInstant(u64);

impl BSPInstant {
    pub fn now() -> Self {
        assert!(is_bsp(), "BSPInstant is only usable from the BSP core");
        Self(tsc::read())
    }

    pub fn add_ticks(self, ticks: u64) -> Self {
        Self(self.0 + ticks)
    }

    pub fn add_ns(self, ns: u64) -> Self {
        Self(self.0 + tsc::ns_to_ticks(ns))
    }

    /// Panics if times in wrong order
    pub fn ticks_from(self, earlier: BSPInstant) -> u64 {
        assert!(earlier <= self);
        self.0 - earlier.0
    }

    /// Panics if times in wrong order
    pub fn duration_from(self, earlier: BSPInstant) -> d7time::Duration {
        d7time::Duration::from_nanos(tsc::ticks_to_ns(self.ticks_from(earlier)))
    }

    pub fn ticks_since(self) -> u64 {
        Self::now().ticks_from(self)
    }

    pub fn duration_since(self) -> d7time::Duration {
        Self::now().duration_from(self)
    }
}
