use core::cmp::Ordering;
use spin::Mutex;

use pit::TIME_BETWEEN_E_12;

#[derive(Copy,Clone)]
pub struct SystemClock {
    seconds: u64,
    nano_fraction: u64
}

impl SystemClock {
    pub const fn new() -> SystemClock {
        SystemClock {seconds: 0, nano_fraction: 0}
    }
    pub fn tick(&mut self) {
        use multitasking::SCHEDULER;

        // Increase current time
        self.nano_fraction += TIME_BETWEEN_E_12/1_000;
        if self.nano_fraction > 1_000_000_000 {
            self.seconds += 1;
            self.nano_fraction -= 1_000_000_000;
        }

        // Update multitasking scheduler
        match SCHEDULER.try_lock() {
            Some(mut s) => s.tick(self.clone()),
            None => {rprintln!("MT: SCHED: Locking failed");}
        }
    }
    pub fn as_microseconds(&self) -> u64 {
        self.seconds*1_000_000 + self.nano_fraction/1_000
    }
    pub fn as_milliseconds(&self) -> u64 {
        self.seconds*1_000 + self.nano_fraction/1_000_000
    }
    pub fn as_seconds(&self) -> u64 {
        self.seconds
    }

    pub fn after_microseconds(&self, delta: u64) -> SystemClock {
        let s = self.seconds + delta / 1_000_000;
        let n = (delta%1_000_000)*1_000;
        SystemClock {seconds: s, nano_fraction: n}
    }
    pub fn after_milliseconds(&self, delta: u64) -> SystemClock {
        let s = self.seconds + delta / 1_000;
        let n = (delta%1_000)*1_000_000;
        SystemClock {seconds: s, nano_fraction: n}
    }
    pub fn after_seconds(&self, delta: u64) -> SystemClock {
        SystemClock {
            seconds: self.seconds+delta,
            nano_fraction: self.nano_fraction
        }
    }
}
impl PartialEq for SystemClock {
    fn eq(&self, other: &SystemClock) -> bool {
        self.as_microseconds() == other.as_microseconds()
    }
}
impl PartialOrd for SystemClock {
    fn partial_cmp(&self, other: &SystemClock) -> Option<Ordering> {
        Some(self.as_microseconds().cmp(&other.as_microseconds()))
    }
}


pub static SYSCLOCK: Mutex<SystemClock> = Mutex::new(SystemClock::new());

pub fn init() {
    rprintln!("SYSCLOCK: enabled");
}

pub fn buzy_sleep_until(until: SystemClock) {
    while SYSCLOCK.lock().as_microseconds() < until.as_microseconds() {}
}

pub fn sleep_ms(ms: u64) {
    let end = SYSCLOCK.lock().after_milliseconds(ms);
    buzy_sleep_until(end);
}
