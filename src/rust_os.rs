#![feature(lang_items)]
#![no_std]

extern crate rlibc;

#[no_mangle]
pub extern "C" fn rust_main() {
    let mut a = ("hello", 42);
    a.1 += 1;
    loop {}
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[cfg(not(test))]
#[lang = "panic_fmt"]
extern "C" fn panic_fmt(fmt: core::fmt::Arguments, file: &str, line: u32) -> ! {
    loop {}
}
