// Code style
#![forbid(private_in_public)]
#![deny(unused_assignments)]

// Code style (development time)
#![allow(unused_macros)]
#![allow(dead_code)]

// Code style (temp)
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(unused_mut)]
#![allow(unused_unsafe)]
#![allow(unreachable_code)]

// Safety
#![deny(overflowing_literals)]
#![deny(safe_packed_borrows)]
#![deny(unused_must_use)]

#![no_std]

#![feature(lang_items)]
#![feature(core_intrinsics)]
#![feature(panic_info_message)]
#![feature(asm)]
#![feature(ptr_internals)]
#![feature(const_fn)]
#![feature(const_vec_new)]
#![feature(naked_functions)]
#![feature(box_syntax, box_patterns)]
#![feature(box_into_raw_non_null)]
#![feature(stmt_expr_attributes)]
#![feature(alloc)]
#![feature(allocator_api)]

use core::alloc::Layout;
use core::panic::PanicInfo;


extern crate volatile;
extern crate rlibc;
extern crate spin;
extern crate x86_64;
extern crate cpuio;
#[macro_use]
extern crate bitflags;
extern crate bit_field;
#[macro_use]
extern crate static_assertions;

extern crate d7alloc;
extern crate d7staticfs;
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
mod virtio;
mod disk_io;
mod staticfs;
// mod ide;
// mod nic;

// Software:
mod elf_parser;
mod time;
mod multitasking;
mod syscall;
mod filesystem;
mod kernel_shell;

/// The kernel main function
#[no_mangle]
pub extern fn rust_main() {
    rreset!();
    rprintln!("Loading the system...\n");

    // Finish system setup

    // Interrupt controller
    pic::init();
    // apic::init();

    // Memory allocation
    memory::init();

    // Interrupt system
    interrupt::init();

    // PIT
    pit::init();

    // CPU data
    cpuid::init();

    // Filesystem
    // filesystem::init();

    // Keyboard
    // keyboard::init();

    // PCI
    pci::init();

    // Disk IO (ATA, IDE, VirtIO)
    disk_io::init();

    // NIC
    // nic::init();

    // rreset!();
    rprintln!("Kernel initialized.\n");

    // Load modules
    if let Some(bytes) = staticfs::read_file("README.md") {
        let mut lines = 3;
        for b in bytes {
            if b == 0x0a {
                lines -= 1;
                if lines == 0 {
                    break;
                }
            }
            if (0x20 <= b && b <= 0x7f)  || b == 0x0a {
                rprint!("{}", b as char);
            }
        }
    } else {
        rprintln!("File not found");
    }


    // use multitasking::PROCMAN;
    //
    // {
    //     let ref mut pm = PROCMAN.lock();
    //     rprintln!("Did not crash!");
    //     let pid = pm.spawn();
    //     rprintln!("PID: {}", pid);
    //     rprintln!("Did not crash!");
    // }

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
extern fn rust_oom(_: Layout) -> ! {
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
#[panic_handler]
#[allow(unused_variables)]
#[no_mangle]
extern "C" fn panic(info: &PanicInfo) -> ! {
    unsafe {
        // asm!("jmp panic"::::"intel","volatile");

        asm!("cli"::::"intel","volatile");
        panic_indicator!(0x4f214f21); // !!

        if let Some(location) = info.location() {
            rprintln!("\nKernel Panic: file: '{}', line: {}", location.file(), location.line());
        } else {
            rprintln!("\nKernel Panic: Location unavailable");
        }
        if let Some(msg) = info.message() {
            rprintln!("  {:?}", msg);
        } else {
            rprintln!("  Info unavailable");
        }
        asm!("jmp panic"::::"intel","volatile");
    }
    loop {}
}
