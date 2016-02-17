#![no_std]
#![feature(lang_items)]
#![feature(unique)]
#![feature(const_fn)]

extern crate rlibc;
extern crate spin;

mod vga_buffer;
mod terminal;
mod util;


/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    use core::fmt::Write;

    let mut tty = terminal::Terminal::new();
    tty.write_byte(b'\n');
    tty.write_str("Charset test: ONLY normal ASCII here: !|&^\\()[]{}?+-*/");
    tty.write_byte(b'\n');

    // memory dump
    let addr: u64 = 0x12000;
    for offset in 0..0x200 {
        if offset%2 == 0 {
            tty.write_byte(b' ');
        }
        if offset%32 == 0 {
            tty.write_byte(b'\n');
        }

        let mut ptr = (addr+offset) as *mut u8;
        let value = unsafe { *ptr };
        let hexval = util::byte_to_hex(value);
        tty.write_byte(hexval[0]);
        tty.write_byte(hexval[1]);
//        tty.write_byte(b' ');
    }

    loop {}
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
