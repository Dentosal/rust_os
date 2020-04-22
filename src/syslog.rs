use alloc::collections::VecDeque;
use alloc::prelude::v1::*;
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};
use log::{Level, Metadata, Record};
use spin::Mutex;

/// Disable logging directly to the built-in vga buffer.
/// This MUST NOT BE done before memory map has been initialized,
/// or it causes page faults. (Requires allocation)
static DISABLE_DIRECT_VGA: AtomicBool = AtomicBool::new(false);

pub fn disable_direct_vga() {
    DISABLE_DIRECT_VGA.store(true, Ordering::Release);
}

/********************************* PORT E9 ***********************************/

struct PortE9;

static mut PORT: cpuio::UnsafePort<u8> = unsafe { cpuio::UnsafePort::new(0xe9) };

/// Allow formatting
impl ::core::fmt::Write for PortE9 {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        unsafe {
            for byte in s.bytes() {
                if byte == b'\n' {
                    PORT.write(b'\r');
                    PORT.write(b'\n');
                } else if 0x20 <= byte && byte <= 0x7e {
                    PORT.write(byte);
                } else {
                    PORT.write(b'?');
                    loop {}
                }
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

/**************************** BUFFER + SYSCALL *******************************/

lazy_static::lazy_static! {
    /// Write ahead log for kernel log messages
    static ref WRITE_AHEAD_LOG: Mutex<VecDeque<u8>> = Mutex::new(VecDeque::new());
}

pub fn syscall_read(buffer: &mut [u8]) -> usize {
    let mut wal = WRITE_AHEAD_LOG.try_lock().unwrap();
    let count = wal.len().min(buffer.len());
    for (i, b) in wal.drain(..count).enumerate() {
        buffer[i] = b;
    }
    count
}

/***************************** LOGGER ITSELF ********************************/

struct SystemLogger;

// pub const LEVEL_SCREEN: log::Level = log::Level::Info;
pub const LEVEL_SCREEN: log::Level = log::Level::Debug;
pub const LEVEL_PORTE9: log::Level = log::Level::Trace;
// pub const LEVEL_PORTE9: log::Level = log::Level::Debug;

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
            if crate::memory::can_allocate() {
                let message = format!(
                    "{:5} {} - {}\n",
                    record.level(),
                    record.target(),
                    record.args()
                );
                let mut wal = WRITE_AHEAD_LOG.try_lock().unwrap();
                wal.extend(message.bytes());
            }

            if !DISABLE_DIRECT_VGA.load(Ordering::Acquire) {
                unsafe {
                    rforce_unlock!(); // TODO: remove
                }
                rprintln!(
                    "{:5} {} - {}",
                    record.level(),
                    record.target(),
                    record.args()
                );
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: SystemLogger = SystemLogger;

pub fn enable() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}
