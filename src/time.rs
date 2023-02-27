use crate::driver::tsc;
use crate::smp::is_bsp;

use core::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TscInstant(u64);

impl TscInstant {
    pub fn now() -> Self {
        Self(tsc::read())
    }

    pub fn tsc_value(self) -> u64 {
        self.0
    }

    pub fn add_ticks(self, ticks: u64) -> Self {
        Self(self.0 + ticks)
    }

    pub fn add_ns(self, ns: u64) -> Self {
        Self(self.0 + crate::smp::sleep::ns_to_ticks(ns))
    }

    pub fn try_ticks_from(self, earlier: TscInstant) -> Option<u64> {
        if earlier <= self {
            Some(self.0 - earlier.0)
        } else {
            None
        }
    }

    /// Panics if times in wrong order
    pub fn ticks_from(self, earlier: TscInstant) -> u64 {
        self.try_ticks_from(earlier)
            .expect("Timestamps in wrong order")
    }

    /// Panics if times in wrong order
    pub fn duration_from(self, earlier: TscInstant) -> Duration {
        Duration::from_nanos(crate::smp::sleep::ticks_to_ns(self.ticks_from(earlier)))
    }

    pub fn ticks_since(self) -> u64 {
        Self::now().ticks_from(self)
    }

    pub fn duration_since(self) -> Duration {
        Self::now().duration_from(self)
    }
}
