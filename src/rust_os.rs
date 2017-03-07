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
#![feature(alloc, collections)]
#![feature(drop_types_in_const)]

extern crate volatile;
extern crate rlibc;
extern crate spin;
extern crate cpuio;
#[macro_use]
extern crate bitflags;

extern crate linked_list_allocator;
extern crate hole_list_allocator;

extern crate alloc;
#[macro_use]
extern crate collections;

// Hardware:
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
// mod cpuid;
mod interrupt;
mod keyboard;
mod pit;
mod memory;
mod pci;
// mod ide;
// mod nic;

// Software:
mod elf_parser;
mod time;
mod multitasking;
mod syscall;

/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    rreset!();
    rprintln!("Loading the system...\n");

    /// Finish system setup

    // interrupt controller
    pic::init();
    // apic::init();

    // interrupt system
    interrupt::init();

    rprintln!("INTr");

    // memory allocation
    memory::init();

    // PIT
    pit::init();

    // cpu data
    // cpuid::init();

    // keyboard
    keyboard::init();

    // PCI
    pci::init();

    // IDE / ATA
    // ide::init();

    // NIC
    // nic::init();

    // Multitasking
    multitasking::init();

    // rreset!();
    rprintln!("Dimension 7 OS");
    rprintln!("\nSystem ready.\n");

    // fn stack_overflow() {
    //     rprint!("."); stack_overflow();
    // }
    // stack_overflow();

    rprintln!("Did not crash!");


    loop {
        use time::{SYSCLOCK, buzy_sleep_until};
        buzy_sleep_until(SYSCLOCK.lock().after_seconds(1));
        rprintln!("JAS");

        let success: bool;
        let result: u64;
        unsafe {
            asm!("
                mov rax, 0x1
                mov rdi, 0x2
                mov rsi, 0x3
                int 0xd7
            " : "={rax}"(success), "={rdx}"(result) :: "eax", "rdx", "rdi", "rsi" : "intel");
        }
        rprintln!("{:?} {:?}", success, result);
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
#[allow(unused_variables)]
#[allow(private_no_mangle_fns)]
#[no_mangle]
extern "C" fn panic_fmt(fmt: core::fmt::Arguments, file: &str, line: u32) -> ! {
    unsafe {
        // asm!("jmp panic"::::"intel","volatile");

        asm!("cli"::::"intel","volatile");
        panic_indicator!(0x4f214f21); // !!

        rprintln!("Kernel Panic: file: '{}', line {}\n", file, line);
        rprintln!("    {}\n", fmt);
        asm!("jmp panic"::::"intel","volatile");
    }
    loop {}
}
