#![no_std]
#![feature(asm)]

pub mod syscall;

use core::panic::PanicInfo;

#[panic_handler]
#[no_mangle]
extern "C" fn panic(_: &PanicInfo) -> ! {
    unsafe {
        asm!("cli"::::"intel","volatile");
        asm!(concat!(
            "mov eax, ", stringify!(0x4f374f21), "; mov [0xb809c], eax") // Error: !7
            ::
            : "eax", "memory"
            : "volatile", "intel"
        );
        asm!("hlt"::::"intel","volatile");
    }
    loop {}
}
