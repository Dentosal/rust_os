use core::cell::UnsafeCell;
use core::intrinsics::likely;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::SeqCst};

use crate::multitasking::{Process, ProcessSwitch, SCHEDULER};

use d7time::{Duration, Instant, TimeSpec};

use crate::driver::pit::TIME_BETWEEN_E_12;

pub struct SystemClock {
    lock: UnsafeCell<AtomicBool>,
    sec: UnsafeCell<AtomicU64>,
    nsec: UnsafeCell<AtomicU32>,
}
unsafe impl Sync for SystemClock {}
impl SystemClock {
    const unsafe fn new() -> Self {
        Self {
            lock: UnsafeCell::new(AtomicBool::new(false)),
            sec: UnsafeCell::new(AtomicU64::new(0)),
            nsec: UnsafeCell::new(AtomicU32::new(0)),
        }
    }

    /// Only to be used by the IRQ handler for PIT clock ticks
    ///
    /// Returns process switch information (optionally)
    ///
    /// # Time constraints
    /// This function must be complete before PIT fires next interrupt,
    /// otherwise it can deadlock.
    ///
    /// ^^^ That shouldn't be possible, as PIT needs EOI before firing again.
    pub unsafe fn tick(&self) -> ProcessSwitch {
        let inc: u32 = (TIME_BETWEEN_E_12 / 1_000) as u32;

        // The lock is only held for clock updates
        let uc_lock = self.lock.get();
        let uc_sec = self.sec.get();
        let uc_nsec = self.nsec.get();

        // Aquire lock
        if (*uc_lock).compare_and_swap(false, true, SeqCst) {
            panic!("SystemClock already locked");
        }

        // Get values
        let mut sec = (*uc_sec).load(SeqCst);
        let mut nsec = (*uc_nsec).load(SeqCst);

        // Cannot overflow, as (2 * max nanoseconds) < u32::MAX
        if nsec + inc >= 1_000_000_000 {
            sec += 1;
            nsec = (nsec + inc) - 1_000_000_000;
        } else {
            nsec += inc;
        }

        // Set new values
        (*uc_sec).store(sec, SeqCst);
        (*uc_nsec).store(nsec, SeqCst);

        // It must not have been updated during this time, no check here
        (*uc_lock).store(false, SeqCst);

        // Update multitasking scheduler
        SCHEDULER.try_lock().expect("SCHED LOCKED").tick(self.now())
    }

    /// Gets current time
    pub fn now(&self) -> Instant {
        unsafe {
            let uc_sec = self.sec.get();
            let uc_nsec = self.nsec.get();

            let mut prev_sec = (*uc_sec).load(SeqCst);
            let mut prev_nsec = (*uc_nsec).load(SeqCst);

            // Polling needed to avoid invalid values on second borders
            loop {
                let sec = (*uc_sec).load(SeqCst);
                let nsec = (*uc_nsec).load(SeqCst);

                if likely(prev_sec == sec && prev_nsec <= nsec) {
                    return Instant::create(TimeSpec { sec, nsec });
                } else {
                    prev_sec = sec;
                    prev_nsec = nsec;
                }
            }
        }
    }
}

pub static SYSCLOCK: SystemClock = unsafe { SystemClock::new() };

pub fn init() {
    rprintln!("SYSCLOCK: enabled");
}

pub fn busy_sleep_until(until: Instant) {
    while SYSCLOCK.now() < until {}
}

pub fn sleep_ms(ms: u64) {
    busy_sleep_until(SYSCLOCK.now() + Duration::from_millis(ms));
}
