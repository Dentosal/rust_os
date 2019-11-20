// Lints
#![forbid(private_in_public)]
#![forbid(bare_trait_objects)]
#![deny(unused_assignments)]
// no_std
#![no_std]
// Unstable features
#![feature(asm)]
#![feature(allocator_api)]
#![feature(const_fn)]
#![feature(integer_atomics)]
#![feature(alloc_error_handler)]
#![feature(panic_info_message)]

mod allocator;
mod kernel_constants;
pub mod syscall;

use core::alloc::Layout;
use core::panic::PanicInfo;

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
    use self::syscall::print_string;
    unsafe {
        if let Some(location) = info.location() {
            print_string(&format!(
                "Error: file '{}', line {}",
                location.file(),
                location.line()
            ));
        } else {
            print_string("Error: (location unavailable)");
        }

        if let Some(msg) = info.message() {
            print_string(&format!("  {:?}", msg));
        } else {
            print_string("  Info unavailable");
        }

        asm!("cli"::::"intel","volatile");
        asm!("hlt"::::"intel","volatile");
    }
    loop {}
}

#[global_allocator]
static HEAP_ALLOCATOR: allocator::GlobAlloc =
    allocator::GlobAlloc::new(allocator::BlockAllocator::new());

#[alloc_error_handler]
fn out_of_memory(_: Layout) -> ! {
    unsafe {
        asm!("cli"::::"intel","volatile");
        asm!("hlt"::::"intel","volatile");
    }
    loop {}
}
