// Code style
#![deny(unused_assignments)]

// Code style (development time)
#![allow(unused_macros)]
#![allow(dead_code)]

// Safety
#![deny(overflowing_literals)]
#![deny(safe_packed_borrows)]

#![no_std]

#![feature(lang_items)]
#![feature(core_intrinsics)]
#![feature(asm)]
#![feature(ptr_internals)]
#![feature(unique)]
#![feature(const_fn)]
#![feature(const_vec_new)]
#![feature(const_generics)]
#![feature(naked_functions)]
#![feature(iterator_step_by)]
// #![feature(box_syntax, box_patterns)]
#![feature(stmt_expr_attributes)]
#![feature(alloc)]
#![feature(global_allocator)]

extern crate volatile;
extern crate rlibc;
extern crate spin;
extern crate x86_64;
extern crate cpuio;
#[macro_use]
extern crate bitflags;
extern crate bit_field;

extern crate d7alloc;
extern crate d7ramfs;

#[macro_use]
extern crate alloc;

// Hardware:
#[macro_use]
mod vga_buffer;
#[macro_use]
mod util;
mod mem_map;
mod paging;
mod acpi;
mod pic;
// mod apic;
mod cpuid;
mod interrupt;
mod keyboard;
mod pit;
mod memory;
mod pci;
mod ata_pio;
// mod ide;
mod nic;

// Software:
mod elf_parser;
mod time;
mod multitasking;
mod syscall;
mod kernel_shell;

/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    rreset!();
    rprintln!("Loading the system...\n");

    // Finish system setup

    // Interrupt controller
    // pic::init();
    // apic::init();

    // Memory allocation
    let mut mem_ctrl = memory::init();

    // Interrupt system
    interrupt::init(&mut mem_ctrl);

    // PIT
    pit::init();

    // cpu data
    cpuid::init();

    // RamFS
    ramfs::init();

    // keyboard
    keyboard::init();

    // ATA PIO
    ata_pio::init();

    // PCI
    pci::init();

    // IDE / ATA
    // ide::init();

    // NIC
    // nic::init();

    // rreset!();
    rprintln!("Kernel initialized.\n");

    // use multitasking::PROCMAN;
    //
    // {
    //     let ref mut pm = PROCMAN.lock();
    //     rprintln!("Did not crash!");
    //     let pid = pm.spawn();
    //     rprintln!("PID: {}", pid);
    //     rprintln!("Did not crash!");
    // }

    unsafe {
        let data = ata_pio::ATA_PIO.lock().read(0, 1);
        use alloc::Vec;
        assert!(data.iter().skip(510).map(|v| *v).collect::<Vec<u8>>() == vec![0x55, 0xAA]);
    }

    kernel_shell::run();

    loop {
        use time::{SYSCLOCK, busy_sleep_until};
        busy_sleep_until(SYSCLOCK.lock().after_seconds(5));
        // rprintln!("Sleep done");

        let success: u64;
        let result: u64;
        unsafe {
            asm!("
                mov rax, 0x1
                mov rdi, 0x2
                mov rsi, 0x3
                int 0xd7
            " : "={rax}"(success), "={rdx}"(result) :: "eax", "rdx", "rdi", "rsi" : "intel");
        }
        // let _ = success;
        // let _ = result;
        rprintln!("{:?} {:?}", success, result);
    }
}

#[global_allocator]
static HEAP_ALLOCATOR: d7alloc::GlobAlloc = d7alloc::GlobAlloc::new(
    d7alloc::BumpAllocator::new(d7alloc::HEAP_START, d7alloc::HEAP_START + d7alloc::HEAP_SIZE)
);

#[lang = "oom"]
#[no_mangle]
#[allow(private_no_mangle_fns)]
extern fn rust_oom() -> ! {
    unsafe {
        asm!("cli"::::"intel","volatile");
        panic_indicator!(0x4f4D4f21); // !M as in "No memory"
        asm!("jmp panic"::::"intel","volatile");
   }
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
