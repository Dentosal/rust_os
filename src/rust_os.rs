#![feature(lang_items)]
#![no_std]

extern crate rlibc;

#[no_mangle]
pub extern fn rust_main() {
    let buffer_ptr = (0xb8000) as *mut _;
    unsafe { *buffer_ptr = 0xbedebead as u32 };
    //
    // loop {}
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[cfg(not(test))]
#[lang = "panic_fmt"]
extern "C" fn panic_fmt(fmt: core::fmt::Arguments, file: &str, line: u32) -> ! {
    let buffer_ptr = (0xb8000) as *mut _;

    unsafe { *buffer_ptr = 0xbedebead as u32 };
    loop {}
}
