//! Temporal quantification.

#![no_std]
#![feature(const_fn)]


use core::fmt;
use core::ops::{Add, AddAssign, Sub, SubAssign};

pub use core::time::Duration;

use serde::{Serialize, Deserialize};

/// Internal time format
/// This is public, but shouldn't be exposed to user applications,
/// only the operating system and maybe some of it's drivers
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TimeSpec {
    pub sec: u64,
    pub nsec: u32,
}
impl TimeSpec {
    #[inline]
    pub const fn new(sec: u64, nsec: u32) -> Self {
        Self { sec, nsec }
    }

    #[inline]
    fn as_duration(&self) -> Duration {
        Duration::new(self.sec, self.nsec)
    }

    pub fn sub_instant(&self, earlier: &Self) -> Duration {
        self.checked_sub_duration(&earlier.as_duration())
            .expect("specified instant was later than self")
            .as_duration()
    }

    pub fn checked_add_duration(&self, duration: &Duration) -> Option<Self> {
        // This can overflow, so checking
        let sec: u64 = self.sec.checked_add(duration.as_secs())?;

        // This cannot overflow, as (2 * max nanoseconds) is always under u32::MAX.
        let nsec: u32 = self.nsec + duration.subsec_nanos();

        if nsec >= 1_000_000_000 {
            Some(Self {
                sec: sec.checked_add(1)?,
                nsec: nsec - 1_000_000_000,
            })
        } else {
            Some(Self { sec, nsec })
        }
    }

    pub fn checked_sub_duration(&self, duration: &Duration) -> Option<Self> {
        let mut sec: u64 = self.sec.checked_sub(duration.as_secs())?;
        let d_nsec = duration.subsec_nanos();

        if self.nsec < d_nsec {
            sec = sec.checked_sub(1)?;
            Some(Self {
                sec,
                nsec: (1_000_000_000 + self.nsec) - d_nsec,
            })
        } else {
            Some(Self {
                sec,
                nsec: self.nsec - d_nsec,
            })
        }
    }
}

/// A measurement of a monotonically nondecreasing clock.
/// Opaque and useful only with `Duration`.
///
/// Instants are always guaranteed to be no less than any previously measured
/// instant when created, and are often useful for tasks such as measuring
/// benchmarks or timing how long an operation takes.
///
/// Note, however, that instants are not guaranteed to be **steady**.  In other
/// words, each tick of the underlying clock may not be the same length (e.g.
/// some seconds may be longer than others). An instant may jump forwards or
/// experience time dilation (slow down or speed up), but it will never go
/// backwards.
///
/// Instants are opaque types that can only be compared to one another. There is
/// no method to get "the number of seconds" from an instant. Instead, it only
/// allows measuring the duration between two instants (or comparing two
/// instants).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Instant(TimeSpec);

/// An error returned from the `duration_since` and `elapsed` methods on
/// `SystemTime`, used to learn how far in the opposite direction a system time
/// lies.
///
/// # Examples
///
/// ```no_run
/// use std::thread::sleep;
/// use std::time::{Duration, SystemTime};
///
/// let sys_time = SystemTime::now();
/// sleep(Duration::from_secs(1));
/// let new_sys_time = SystemTime::now();
/// match sys_time.duration_since(new_sys_time) {
///     Ok(_) => {}
///     Err(e) => println!("SystemTimeError difference: {:?}", e.duration()),
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemTimeError(Duration);

impl Instant {
    /// Used by the system clock as constructor
    ///
    /// # Safety
    ///
    /// Unsafe to prevent accidental use, as well as to remind about
    /// the monotonicity guarantees
    #[inline]
    pub unsafe fn create(ts: TimeSpec) -> Self {
        Self(ts)
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
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.0.sub_instant(&earlier.0)
    }

    /// Returns `Some(t)` where `t` is the time `self + duration` if `t` can be represented as
    /// `Instant` (which means it's inside the bounds of the underlying data structure), `None`
    /// otherwise.
    pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
        self.0.checked_add_duration(&duration).map(Instant)
    }

    /// Returns `Some(t)` where `t` is the time `self - duration` if `t` can be represented as
    /// `Instant` (which means it's inside the bounds of the underlying data structure), `None`
    /// otherwise.
    pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
        self.0.checked_sub_duration(&duration).map(Instant)
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    /// # Panics
    ///
    /// This function may panic if the resulting point in time cannot be represented by the
    /// underlying data structure. See [`checked_add`] for a version without panic.
    ///
    /// [`checked_add`]: ../../std/time/struct.Instant.html#method.checked_add
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

impl fmt::Debug for Instant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
