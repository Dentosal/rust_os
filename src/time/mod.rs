use spin::Mutex;

use pit::TIME_BETWEEN_E_12;

pub struct SystemClock {
    seconds: u64,
    nano_fraction: u64
}

impl SystemClock {
    pub const fn new() -> SystemClock {
        SystemClock {seconds: 0, nano_fraction: 0}
    }
    pub unsafe fn tick(&mut self) {
        self.nano_fraction += TIME_BETWEEN_E_12/1_000;
        if self.nano_fraction > 1_000_000_000 {
            self.seconds += 1;
            self.nano_fraction -= 1_000_000_000;
        }
    }
    pub fn now_microseconds(&self) -> u64 {
        self.seconds*1_000_000 + self.nano_fraction/1_000
    }
    pub fn now_milliseconds(&self) -> u64 {
        self.seconds*1_000 + self.nano_fraction/1_000_000
    }
    pub fn now_seconds(&self) -> u64 {
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


pub static SYSCLOCK: Mutex<SystemClock> = Mutex::new(SystemClock::new());

pub fn init() {
    rprintln!("SYSCLOCK: enabled");
}

pub fn sleep_until(until: SystemClock) {
    let u_micro: u64 = until.now_microseconds();
    loop {
        let n_micro = SYSCLOCK.lock().now_microseconds();
        if n_micro >= u_micro {
            break;
        }
    }
}
