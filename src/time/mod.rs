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
}


pub static SYSCLOCK: Mutex<SystemClock> = Mutex::new(SystemClock::new());

pub fn init() {
    rprintln!("SYSCLOCK: enabled");
}
