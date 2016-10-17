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

use spin::Mutex;

pub use interrupt::{keyboard_event};
use mem_map::{FrameAllocator, BitmapAllocator};

/// Display startup message
fn display_message() {
    rreset!();
    rprintln!("Dimension 7 OS\n");
}

/// Finish system setup
fn environment_setup() {
    // frame allocator
    mem_map::create_memory_bitmap();

    // interrupt controller
    pic::init();

    // interrupt system
    interrupt::init();

    // cpu feature detection (must be after interrupt handler, if the cpuid instruction is not supported => invalid opcode exception)
    // cpuid::init(); // currently disabled

    // paging


}

/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    display_message();
    environment_setup();

    unsafe {
        // asm!("int 0"::::"intel"); // int0 (#DE)
        // asm!("xor eax, eax; div eax;"::::"intel"); // #DE
        // asm!("ud2"); // invalid opcode
        // *(0xdeadbeef as *mut u64) = 42; // #PF
        int!(3);
    }

    rprintln!("OK?!");
    loop {}

    // let mut frame_allocator = ALLOCATOR.lock();
    let mut frame_allocator = ALLOCATOR!();
    paging::test_paging(&mut frame_allocator);

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
        panic_indicator!(0x4f214f21); // !!
        // vga_buffer::panic_output(format_args!("Kernel Panic: file: '{}', line {}\n", file, line));
        // vga_buffer::panic_output(format_args!("    {}\n", fmt));
        // asm!("jmp panic"::::"intel");
    }
    loop {}
}
