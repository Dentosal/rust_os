use core::arch::asm;
use core::ops::{Add, AddAssign, Sub, SubAssign};

// Re-exports
pub use chrono;
pub use core::time::Duration;

/// Monotonic and steady per-process instant.
/// Opaque and useful only with `Duration`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(u64);

impl Instant {
    pub fn now() -> Instant {
        let rdx: u64;
        let rax: u64;
        let tsc_offset: u64;
        unsafe {
            // Read TSC value and processor_id
            let rcx: u64;
            asm!(
                "rdtscp",
                out("rdx") rdx,
                out("rax") rax,
                out("rcx") rcx,
                options(nomem, nostack)
            );
            // Retrieve info is for the cpu that gave us the timestamp,
            // even if a process switch occurs during this function.
            tsc_offset = d7abi::processor_info::read(rcx as u32).tsc_offset;
        }

        let tsc_timestamp = (rdx << 32) | (rax & 0xffff_ffff);
        Self(tsc_timestamp.wrapping_add(tsc_offset))
    }

    /// Returns current frequency in Hz. Process switch can occur
    /// immediately after this operation making the result somewhat invalid.
    /// However, this is not currently a big issue since:
    /// * Process switches are relatively rare
    /// * All CPU cores likely have same TSC frequency anyway.
    /// * We do no support multi-socket systems yet.
    fn freq_hz() -> u64 {
        // Safety: Safe, this is in userspace
        unsafe { d7abi::processor_info::read_current().tsc_freq_hz }
    }

    #[inline]
    fn duration_from_ticks(ticks: u64) -> Duration {
        let freq = Self::freq_hz();
        let freq_khz = freq / 1000;

        // Separate full seconds
        let secs = ticks / freq;
        let ticks = ticks % freq;

        // Nanoseconds
        let ns = ticks.checked_mul(1_000_000).unwrap() / freq_khz;

        Duration::new(secs, ns as u32)
    }

    #[inline]
    fn duration_to_tics_checked(duration: Duration) -> Option<u64> {
        let freq = Self::freq_hz();
        let inc_a = duration.as_secs().checked_mul(freq)?;

        // duration.subsec_nanos() * freq fits into u64 iff (freq < 18GhZ)
        // not too future-proof, if we are optimistic about CPUs, so using kHz here
        let freq_khz = freq / 1000;
        let inc_b = (duration.subsec_nanos() as u64)
            .checked_mul(freq_khz)
            .unwrap()
            / 1_000_000;

        inc_a.checked_add(inc_b)
    }

    pub fn checked_add(&self, duration: Duration) -> Option<Self> {
        let v = Self::duration_to_tics_checked(duration)?;
        Some(Self(self.0.checked_add(v)?))
    }

    pub fn checked_sub(&self, duration: Duration) -> Option<Self> {
        let v = Self::duration_to_tics_checked(duration)?;
        Some(Self(self.0.checked_sub(v)?))
    }

    /// Returns the amount of time elapsed from another instant to this one.
    ///
    /// # Panics
    ///
    /// This function will panic if `earlier` is later than `self`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::time::{Duration, Instant};
    /// use std::thread::sleep;
    ///
    /// let now = Instant::now();
    /// sleep(Duration::new(1, 0));
    /// let new_now = Instant::now();
    /// println!("{:?}", new_now.duration_since(now));
    /// ```
    pub fn duration_since(&self, earlier: Self) -> Duration {
        let duration_ticks = self.0.checked_sub(earlier.0).expect("duration since later");
        if duration_ticks == 0 {
            return Duration::ZERO;
        }
        Self::duration_from_ticks(duration_ticks)
    }

    /// Time elapsed since this instant
    pub fn elapsed(&self) -> Duration {
        Self::now().duration_since(*self)
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    /// # Panics
    ///
    /// This function may panic if the resulting point in time cannot be represented by the
    /// underlying data structure. Use `checked_add` for a version without panic.
    fn add(self, other: Duration) -> Instant {
        self.checked_add(other)
            .expect("overflow when adding duration to instant")
    }
}

impl AddAssign<Duration> for Instant {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, other: Duration) -> Instant {
        self.checked_sub(other)
            .expect("overflow when subtracting duration from instant")
    }
}

impl SubAssign<Duration> for Instant {
    fn sub_assign(&mut self, other: Duration) {
        *self = *self - other;
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    fn sub(self, other: Instant) -> Duration {
        self.duration_since(other)
    }
}
