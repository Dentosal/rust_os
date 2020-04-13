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
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![feature(asm)]
#![feature(const_fn)]
#![feature(integer_atomics)]
#![feature(panic_info_message)]

mod allocator;
mod select;

pub mod attachment;
pub mod console;
pub mod fs;
pub mod net;
pub mod process;
pub mod syscall;

use core::alloc::Layout;
use core::panic::PanicInfo;

pub use d7abi;
pub use pinecone;

#[macro_use]
extern crate alloc;

extern "Rust" {
    fn main() -> u64;
}

#[no_mangle]
pub extern "C" fn _start() {
    let return_code = unsafe { main() };
    self::syscall::exit(return_code);
}

#[panic_handler]
#[no_mangle]
extern "C" fn panic(info: &PanicInfo) -> ! {
    use self::syscall::debug_print;
    if let Some(location) = info.location() {
        let _ = debug_print(&format!(
            "Error: file '{}', line {}",
            location.file(),
            location.line()
        ));
    } else {
        let _ = debug_print("Error: (location unavailable)");
    }

    if let Some(msg) = info.message() {
        let _ = debug_print(&format!("  {:?}", msg));
    } else {
        let _ = debug_print("  Info unavailable");
    }

    syscall::exit(1)
}

#[global_allocator]
static HEAP_ALLOCATOR: allocator::GlobAlloc =
    allocator::GlobAlloc::new(allocator::BlockAllocator::new());

#[alloc_error_handler]
fn out_of_memory(_: Layout) -> ! {
    unsafe {
        asm!("xchg bx, bx"::::"intel","volatile");
        asm!("cli"::::"intel","volatile");
        asm!("hlt"::::"intel","volatile");
    }
    loop {}
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