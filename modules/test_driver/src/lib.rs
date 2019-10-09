#![no_std]
#![feature(asm)]

use core::panic::PanicInfo;

use d7abi::syscall;

#[no_mangle]
pub extern "C" fn main() {
    // Test: addition
    let mut success: u64;
    let mut result: u64;
    unsafe {
        asm!("mov rax, 0x01; mov rdi, 2; mov rsi, 3; int 0xd7" : "={rax}"(success), "={rdi}"(result) :: "rsi" : "intel");
    }

    if success != 0 || result != 5 {
        unsafe {
            asm!("xchg bx, bx" :::: "volatile", "intel");
        }
    }

    syscall::exit(0);

}

#[panic_handler]
#[no_mangle]
extern "C" fn panic(_: &PanicInfo) -> ! {
    loop {}
}
