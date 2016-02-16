#![no_std]
#![feature(lang_items)]
#![feature(unique)]
#![feature(const_fn)]

extern crate rlibc;
extern crate spin;

mod vga_buffer;
mod terminal;


/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    use core::fmt::Write;

    let mut tty = terminal::Terminal::new();

    tty.write_byte(b'x');
    tty.write_byte(b'y');
    tty.write_byte(b'\n');
    tty.write_str("Test");

    // FIXME: causes guru meditation: write!(tty, ", some numbers: {} {}", 42, 1.337);

    loop{}
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
