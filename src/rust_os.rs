#![allow(dead_code)]

#![no_std]
#![feature(lang_items)]
#![feature(asm)]
#![feature(unique)]
#![feature(const_fn)]
#![feature(step_by)]
#![feature(inclusive_range_syntax)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]
#![feature(stmt_expr_attributes)]

extern crate rlibc;
extern crate spin;
extern crate cpuio;
#[macro_use]
extern crate bitflags;



#[macro_use]
mod vga_buffer;
#[macro_use]
mod util;
#[macro_use]
mod mem_map;
mod paging;
mod pic;
mod cpuid;
mod interrupt;
mod keyboard;
mod elf_parser;

use spin::Mutex;

use keyboard::{KEYBOARD,Keyboard,KeyboardEvent};


/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    rreset!();
    rprintln!("Dimension 7 OS");
    rprintln!("Just a millisecond, loading the system...\n");

    /// Finish system setup

    // receive raw kernel elf image data before we allow overwriting it
    let kernel_elf_header =  unsafe { elf_parser::parse_kernel_elf() };

    // frame allocator
    mem_map::create_memory_bitmap();

    // interrupt controller
    pic::init();

    // keyboard
    keyboard::init();


    rprintln!("??");loop {}


    // interrupt system
    interrupt::init();

    // cpu feature detection (must be after interrupt handler, if the cpuid instruction is not supported => invalid opcode exception)
    cpuid::init(); // currently disabled


    // paging
    paging::init();

    // Test stuff

    rprintln!("Diving in...");

    // loop {}
    paging::test_paging();

    // hang
    rprintln!("\nSystem ready.\n");
    loop {}
}


#[cfg(not(test))]
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {loop {}}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern "C" fn eh_personality() -> ! {loop {}}

#[cfg(not(test))]
#[lang = "panic_fmt"]
#[allow(unused_variables)]
extern "C" fn panic_fmt(fmt: core::fmt::Arguments, file: &str, line: u32) -> ! {
    unsafe {
        asm!("cli"::::"intel","volatile");
        panic_indicator!(0x4f214f21); // !!
        vga_buffer::panic_output(format_args!("Kernel Panic: file: '{}', line {}\n", file, line));
        vga_buffer::panic_output(format_args!("    {}\n", fmt));
        // asm!("jmp panic"::::"intel","volatile");
    }
    loop {}
}
