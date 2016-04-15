#![no_std]
#![feature(lang_items)]
#![feature(asm)]
#![feature(unique)]
#![feature(const_fn)]
#![feature(braced_empty_structs)]
#![feature(step_by)]

extern crate rlibc;
extern crate spin;
extern crate cpuio;
extern crate bitflags;

#[macro_use]
mod vga_buffer;
mod util;
mod mem_map;
mod pic;

use vga_buffer::{Color, CellColor};

/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    // startup message
    rreset!();
    rprintln!("Dimension 7 OS\n");
    rprintln!("Initializing system...");
    rprintln!("");

    // set up frame allocator
    mem_map::create_memory_bitmap();

    // pic setup
    unsafe {
        pic::PICS.lock().init();
    }



    // paging


    // hang
    rprintln!("");
    rprintln!("System ready.");
    loop {}
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[cfg(not(test))]
#[lang = "panic_fmt"]
extern "C" fn panic_fmt(fmt: core::fmt::Arguments, file: &str, line: u32) -> ! {
    // unsafe {
    //     asm!("jmp panic"::::"intel");
    // }
    // rreset!();
    rprintln!("Kernel Panic: file: '{}', line {}", file, line);
    rprintln!("    {}\n", fmt);
    loop {}
}
