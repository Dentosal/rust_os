#![no_std]
#![feature(asm)]

pub mod syscall;

use core::panic::PanicInfo;

#[panic_handler]
#[no_mangle]
extern "C" fn panic(_: &PanicInfo) -> ! {
    loop {}
}
