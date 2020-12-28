use alloc::collections::VecDeque;
use alloc::prelude::v1::*;
use core::fmt::Write;
use core::sync::atomic::{spin_loop_hint, AtomicBool, Ordering};
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

static PORT: Mutex<cpuio::UnsafePort<u8>> = Mutex::new(unsafe { cpuio::UnsafePort::new(0xe9) });

/// Allow formatting
impl ::core::fmt::Write for PortE9 {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        let mut port = PORT.lock();
        unsafe {
            for byte in s.bytes() {
                assert!(byte != 0);
                if byte == b'\n' {
                    port.write(b'\r');
                    port.write(b'\n');
                } else if 0x20 <= byte && byte <= 0x7e {
                    port.write(byte);
                } else {
                    port.write(b'?');
                    bochs_magic_bp!();
                    loop {
                        unsafe {
                            asm!("cli; hlt");
                        }
                    }
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

/******************************* UART SERIAL *********************************/

static UART_LOCK: AtomicBool = AtomicBool::new(false);

struct Uart;

/// Allow formatting
impl ::core::fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        use crate::driver::uart::{has_com1, write_com1};
        if has_com1() {
            // Acquire lock
            while !UART_LOCK.compare_and_swap(false, true, Ordering::SeqCst) {
                spin_loop_hint();
            }

            for byte in s.bytes() {
                assert!(byte != 0);
                if byte == b'\n' {
                    write_com1(b'\r');
                    write_com1(b'\n');
                } else if 0x20 <= byte && byte <= 0x7e {
                    write_com1(byte);
                } else {
                    write_com1(b'?');
                    bochs_magic_bp!();
                    loop {
                        unsafe {
                            asm!("cli; hlt");
                        }
                    }
                }
            }

            // Release lock
            UART_LOCK.store(false, Ordering::SeqCst);
        }
        Ok(()) // Success. Always.
    }
}

static mut UART: Uart = Uart;

macro_rules! uart_print {
    ($fmt:expr, $($arg:tt)*) => (
        unsafe {
            UART.write_fmt(format_args!(concat!($fmt, "\n"), $($arg)*)).unwrap()
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
                "{:30} [{}] {:5} {}",
                record.target(),
                crate::smp::current_processor_id(),
                record.level(),
                record.args()
            );

            uart_print!(
                "{:30} [{}] {:5} {}",
                record.target(),
                crate::smp::current_processor_id(),
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
                let mut wal = WRITE_AHEAD_LOG.lock();
                wal.extend(message.bytes());
            }

            if !DISABLE_DIRECT_VGA.load(Ordering::Acquire) {
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
