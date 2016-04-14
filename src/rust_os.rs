#![no_std]
#![feature(lang_items)]
#![feature(unique)]
#![feature(const_fn)]
#![feature(asm)]

extern crate rlibc;
extern crate spin;


#[macro_use]
mod vga_buffer;
mod util;
mod ram_map;

use vga_buffer::{Color, CellColor};

/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    use core::fmt::Write;

    // test
    rreset!();
    rprintln!("Tuubaaja");

    // startup message
    rprintln!("Dimension 7 OS\n");
    rprintln!("Initializing system...");


    // read memory map
    let mmap_ok = ram_map::load_memory_map();

    // hang
    rprintln!("System ready.");
    loop {}
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[cfg(not(test))]
#[lang = "panic_fmt"]
// VirtualBox does not like panics, and most likely guru meditates instead
extern "C" fn panic_fmt(fmt: core::fmt::Arguments, file: &str, line: u32) -> ! {
    // unsafe {
    //     asm!("jmp panic"::::"intel");
    // }
    rreset!();
    rprintln!("Kernel Panic: file: '{}', line {}", file, line);
    rprintln!("    {}\n", fmt);
    loop {}
}
