// Lints
#![forbid(private_in_public)]
#![forbid(bare_trait_objects)]
#![deny(unused_must_use)]
#![deny(unused_assignments)]
#![deny(clippy::missing_safety_doc)]
#![allow(clippy::empty_loop)]
// no_std
#![no_std]
// Unstable features
#![feature(alloc_error_handler)]
#![feature(allocator_api)]
#![feature(slice_ptr_get)]
#![feature(nonnull_slice_from_raw_parts)]
#![feature(integer_atomics)]
#![feature(panic_info_message)]
#![feature(trait_alias, type_alias_impl_trait)]
#![feature(stmt_expr_attributes)]
#![feature(never_type)]
#![feature(int_roundings)]

mod allocator;

// pub mod console;
pub mod env;
pub mod ipc;
pub mod net;
pub mod process;
pub mod random;
pub mod service;
pub mod syscall;
pub mod time;

use core::alloc::Layout;
use core::arch::asm;
use core::panic::PanicInfo;

pub use d7abi;
pub use pinecone;
pub use x86_64::{self, PhysAddr, VirtAddr};

#[macro_use]
extern crate alloc;

extern "Rust" {
    fn main() -> u64;
}

use log::{Level, LevelFilter, Metadata, Record};

struct SimpleLogger;

const LOG_LEVEL: Level = Level::Debug;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= LOG_LEVEL
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let t = record.target();
            let target_module = t.split_once("::").map(|(a, _)| a).unwrap_or(t);

            println!(
                "{:20} {} - {}",
                target_module,
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

#[no_mangle]
pub extern "C" fn _start() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Trace))
        .expect("Logger error");

    let return_code = unsafe { main() };
    self::syscall::exit(return_code);
}

#[panic_handler]
#[no_mangle]
extern "C" fn panic(info: &PanicInfo) -> ! {
    use self::syscall::debug_print;

    let _ = debug_print("Panic! (attempting allocation to show error the message)");

    let no_location = format!("(location unavailable)");
    let location = info
        .location()
        .map(|l| format!("file '{}', line {}", l.file(), l.line()))
        .unwrap_or(no_location);

    let no_args = format_args!("(message unavailable)");
    let message = format!("  {:?}", info.message().unwrap_or(&no_args));

    let _ = debug_print(&format!("Error: {}\n  {}", location, message));

    syscall::exit(1)
}

#[global_allocator]
static HEAP_ALLOCATOR: allocator::GlobAlloc =
    allocator::GlobAlloc::new(allocator::BlockAllocator::new());

#[alloc_error_handler]
fn out_of_memory(_: Layout) -> ! {
    unsafe {
        asm!("xchg bx, bx");
        loop {
            asm!("cli; hlt");
        }
    }
}

// Output macros
// TODO: print to stdout and not debug_print

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => ({
        $crate::syscall::debug_print(&format!("{}", &format_args!($($arg)*)));
    });
}

// #[macro_export]
// macro_rules! print {
//     ($($arg:tt)*) => ({
//         $crate::syscall::debug_print(&format!("{}", &format_args!($($arg)*)));
//     });
// }

// #[macro_export]
// macro_rules! println {
//     ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
//     ($fmt:expr, $($arg:tt)*) => ($crate::print!(concat!($fmt, "\n"), $($arg)*));
// }
