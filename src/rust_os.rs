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
extern crate bitflags;



#[macro_use]
mod vga_buffer;
mod util;
mod mem_map;
mod pic;
mod cpuid;
mod interrupt;
mod keyboard;

pub use interrupt::{keyboard_event};

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

    // rprintln!("??"); unsafe {asm!("jmp breakpoint"::::"intel","volatile");};

    // unsafe {asm!("mov WORD PTR [0xb8000], 0xbeef"::::"intel","volatile");}

    // Initializing modules
    pic::init();
    interrupt::init();
    // cpuid::init(); // must be after interrupt handler, if the cpuid instruction is not supported => invalid opcode exception



    // unsafe {
    //     asm!("int 0"::::"intel");
    //     asm!("xor eax, eax; div eax;"::::"intel");
    // }

    // paging


    // hang
    rprintln!("");
    rprintln!("System ready.");
    loop {
        // rprint!(".");
        // for _ in 0..1000000 {
        //     unsafe {
        //         asm!("nop" :::: "volatile", "intel")
        //     }
        // }
    }
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
extern "C" fn panic_fmt(fmt: core::fmt::Arguments, file: &str, line: u32) -> ! {
    unsafe {
        panic_indicator!(0x4f214f21); // !!
        // vga_buffer::panic_output(format_args!("Kernel Panic: file: '{}', line {}\n", file, line));
        // vga_buffer::panic_output(format_args!("    {}\n", fmt));
        // asm!("jmp panic"::::"intel");
    }
    loop {}
}
