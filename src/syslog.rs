use core::fmt::Write;
use log::{Level, Metadata, Record};

struct PortE9;

/// Allow formatting
impl ::core::fmt::Write for PortE9 {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        unsafe {
            let mut port = cpuio::UnsafePort::<u8>::new(0xe9);
            for byte in s.bytes() {
                if byte == b'\n' {
                    port.write(b'\r');
                }
                port.write(byte);
            }
        }
        Ok(()) // Success. Always.
    }
}

static mut PORT_E9: PortE9 = PortE9;

macro_rules! e9_print {
    ($fmt:expr, $($arg:tt)*) => (
        unsafe {
            PORT_E9.write_fmt(format_args!(concat!($fmt, "\n"), $($arg)*)).unwrap()
        }
    );
}

struct SystemLogger;

pub const LEVEL_SCREEN: log::Level = log::Level::Info;
pub const LEVEL_PORTE9: log::Level = log::Level::Trace;

impl log::Log for SystemLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= LEVEL_SCREEN || metadata.level() <= LEVEL_PORTE9
    }

    fn log(&self, record: &Record) {
        let level = record.metadata().level();
        if level <= LEVEL_PORTE9 {
            e9_print!(
                "{:40} {:5}  {}",
                record.target(),
                record.level(),
                record.args()
            );
        }
        if level <= LEVEL_SCREEN {
            unsafe {
                rforce_unlock!();
            }
            rprintln!(
                "{:5} {} - {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

static LOGGER: SystemLogger = SystemLogger;

pub fn enable() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
