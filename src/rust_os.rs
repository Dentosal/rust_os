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


extern crate volatile;
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
mod acpi;
mod pic;
// mod apic;
mod cpuid;
mod interrupt;
mod keyboard;
mod elf_parser;

use spin::Mutex;

// use keyboard::{KEYBOARD,Keyboard,KeyboardEvent};


/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    rreset!();
    rprintln!("Dimension 7 OS");
    rprintln!("Just a millisecond, loading the system...\n");

    /// Finish system setup

    // interrupt system
    // interrupt::init();

    // receive raw kernel elf image data before we allow overwriting it
    let kernel_elf_header =  unsafe { elf_parser::parse_kernel_elf() };


    // frame allocator
    mem_map::create_memory_bitmap();

    rprintln!("BRPT"); loop {}
    // cpu data
    // cpuid::init();

    // interrupt controller
    pic::init();
    // apic::init();



    // keyboard
    //keyboard::init();

    rprintln!("Init ok."); loop {}

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
#[no_mangle]
extern "C" fn panic_fmt(fmt: core::fmt::Arguments, file: &str, line: u32) -> ! {
    unsafe {
        asm!("jmp panic"::::"intel","volatile");

        asm!("cli"::::"intel","volatile");
        panic_indicator!(0x4f214f21); // !!

        rprintln!("Kernel Panic: file: '{}', line {}\n", file, line);
        rprintln!("    {}\n", fmt);
//        asm!("jmp panic"::::"intel","volatile");
    }
    loop {}
}
