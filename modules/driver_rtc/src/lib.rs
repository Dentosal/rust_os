//! CMOS RTC support

#![no_std]
#![feature(allocator_api)]
#![deny(unused_must_use)]

extern crate alloc;
extern crate libd7;

use core::arch::asm;
use cpuio::UnsafePort;
use libd7::time::chrono::{NaiveDate, NaiveDateTime};
use libd7::{ipc, select};

const NMI_DISABLE_BIT: u8 = 1 << 7;
const PORT_REGSEL: UnsafePort<u8> = unsafe { cpuio::UnsafePort::new(0x70) };
const PORT_VALUE: UnsafePort<u8> = unsafe { cpuio::UnsafePort::new(0x71) };

fn read_register(register_number: u8) -> u8 {
    assert!(register_number < (u8::MAX & !NMI_DISABLE_BIT));
    // Safety: register numbers checked by the above asserts
    unsafe {
        // Select port
        PORT_REGSEL.write(NMI_DISABLE_BIT | register_number);
        PORT_VALUE.read()
    }
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Binary,
    Bcd,
}
impl Mode {
    /// https://wiki.osdev.org/CMOS#Format_of_Bytes
    fn to_bin(self, v: u8) -> u8 {
        match self {
            Self::Binary => v,
            Self::Bcd => ((v & 0xf0) >> 1) + ((v & 0xf0) >> 3) + (v & 0xf),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum HoursMode {
    _12,
    _24,
}
impl HoursMode {
    fn to_24h(self, v: u8) -> u8 {
        match self {
            Self::_24 => v,
            Self::_12 => {
                let pm = v & (1 << 7) != 0;
                let m = v & !(1 << 7);
                assert!(m != 0); // 12h clock has no zero
                assert!(m <= 12);
                if pm {
                    if m == 12 { 12 } else { m + 12 }
                } else {
                    if m == 12 { 0 } else { m }
                }
            },
        }
    }
}

/// https://wiki.osdev.org/CMOS#Format_of_Bytes
fn read_config() -> (Mode, HoursMode) {
    let b = read_register(0x0b);
    (
        if b & (1 << 2) != 0 {
            Mode::Binary
        } else {
            Mode::Bcd
        },
        if b & (1 << 1) != 0 {
            HoursMode::_24
        } else {
            HoursMode::_12
        },
    )
}

/// https://wiki.osdev.org/CMOS#RTC_Update_In_Progress
fn is_update_in_progress() -> bool {
    read_register(0x0a) & (1 << 7) != 0
}

#[derive(Clone, Copy, PartialEq)]
struct TimeRegisterSnapshot {
    seconds: u8,
    minutes: u8,
    hours: u8,
    day: u8,
    month: u8,
    year_last_digits: u8,
    maybe_century: u8,
}
impl TimeRegisterSnapshot {
    fn read((mode, hoursf): (Mode, HoursMode)) -> Self {
        Self {
            seconds: mode.to_bin(read_register(0x00)),
            minutes: mode.to_bin(read_register(0x02)),
            hours: hoursf.to_24h(mode.to_bin(read_register(0x04))),
            day: mode.to_bin(read_register(0x07)),
            month: mode.to_bin(read_register(0x08)),
            year_last_digits: mode.to_bin(read_register(0x09)),
            maybe_century: mode.to_bin(read_register(0x32)),
        }
    }
}

/// https://wiki.osdev.org/CMOS#Getting_Current_Date_and_Time_from_RTC
/// Timezone of RTC is not known, so this returns `NaiveDateTime`.
/// On some platforms (notably emulators) it is knwon, so...
/// TODO: if timezone is known, include it in the result
fn get_current_time(config: (Mode, HoursMode)) -> NaiveDateTime {
    log::debug!("Reading RTC value");
    // Do the reading in a tight loop with interrupts disabled,
    // to make sure we read the value consistently, even when
    // the CMOS chip updates the value during the read
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
    let t = loop {
        if is_update_in_progress() {
            continue;
        }
        let a = TimeRegisterSnapshot::read(config);
        if is_update_in_progress() {
            continue;
        }
        let b = TimeRegisterSnapshot::read(config);
        if a == b {
            break a;
        }
    };
    unsafe {
        asm!("sti", options(nomem, nostack));
    }

    assert!(t.seconds < 60); // Do not allow leap seconds
    assert!(t.minutes < 60);
    assert!(t.hours < 24);
    assert!(t.day != 0 && t.day <= 31);
    assert!(t.month != 0 && t.month <= 12);

    let year = if t.maybe_century == 0 {
        2000 + (t.year_last_digits as u16)
    } else {
        log::debug!("maybe_century = {}", t.maybe_century);
        assert!(t.maybe_century >= 20);
        100 * (t.maybe_century as u16) + (t.year_last_digits as u16)
    };

    NaiveDate::from_ymd(year as i32, t.month as u32, t.day as u32).and_hms(
        t.hours as u32,
        t.minutes as u32,
        t.seconds as u32,
    )
}

#[no_mangle]
fn main() -> ! {
    log::debug!("RTC driver starting");

    let config = read_config();

    log::trace!("RTC clock configuration {:?}", config);
    log::trace!("RTC time on startup {}", get_current_time(config));

    // Subscribe to read requests
    let read_time: ipc::Server<(), NaiveDateTime> = ipc::Server::exact("rtc/read").unwrap();

    // Inform serviced that we are running.
    libd7::service::register("driver_rtc", false);

    loop {
        select! {
            one(read_time) => {
                // Ignore errors
                let _ = read_time.handle(|()| Ok(get_current_time(config)));
            }
        }
    }
}
