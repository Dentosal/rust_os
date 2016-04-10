#![no_std]
#![feature(lang_items)]
#![feature(unique)]
#![feature(const_fn)]
#![feature(asm)]

extern crate rlibc;
extern crate spin;

mod vga_buffer;
mod terminal;
mod util;

use vga_buffer::{Color, CellColor};

/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    use core::fmt::Write;

    let mut tty = terminal::TERMINAL.lock();
    tty.set_color(CellColor::new(Color::Green, Color::Black));
    tty.clear();

    tty.write_str("Bootup complete.\n");
    tty.newline();
    tty.write_str("Dimension 7 OS\n");
    tty.write_str("Copyright (c) 2016 Hannes Karppila\n");


    loop {}
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[cfg(not(test))]
#[lang = "panic_fmt"]
extern "C" fn panic_fmt(fmt: core::fmt::Arguments, file: &str, line: u32) -> ! {
    unsafe {
        asm!("jmp error"::::"intel");
    }
    loop {}
}
