// Code style
#![forbid(private_in_public)]
#![forbid(tyvar_behind_raw_pointer)]
#![deny(unused_assignments)]
#![allow(clippy::inconsistent_digit_grouping)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::empty_loop)]
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
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::cast_ptr_alignment)]
#![allow(clippy::identity_op)]
// Safety
#![deny(overflowing_literals)]
#![deny(safe_packed_borrows)]
#![deny(unused_must_use)]
// No-std
#![no_std]
// Unstable features
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![feature(asm)]
#![feature(box_into_raw_non_null)]
#![feature(box_syntax, box_patterns)]
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![feature(integer_atomics)]
#![feature(lang_items)]
#![feature(maybe_uninit_extra)]
#![feature(naked_functions)]
#![feature(no_more_cas)]
#![feature(panic_info_message)]
#![feature(ptr_internals)]
#![feature(stmt_expr_attributes)]
#![feature(trait_alias)]
#![feature(try_trait)]

use core::alloc::Layout;
use core::panic::PanicInfo;

extern crate cpuio;
extern crate rlibc;
extern crate spin;
extern crate volatile;
extern crate x86_64;
#[macro_use]
extern crate bitflags;
extern crate bit_field;
#[macro_use]
extern crate static_assertions;

extern crate d7alloc;
extern crate d7ramfs;
extern crate d7staticfs;
extern crate d7time;

#[macro_use]
extern crate alloc;

// Utilities and macros:
#[macro_use]
mod util;

// Hardware drivers
#[macro_use]
mod driver;

// Everything else
mod cpuid;
mod filesystem;
mod interrupt;
mod kernel_shell;
mod memory;
mod multitasking;
mod syscall;
mod time;

/// The kernel main function
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    rreset!();
    rprintln!("Loading the system...\n");

    // Finish system setup

    // Interrupt controller
    driver::pic::init();
    // apic::init();

    // Interrupt system
    interrupt::init();

    // Memory allocation and paging
    memory::init();

    // More interrupts controls
    interrupt::init_after_memory();

    // PIT
    driver::pit::init();

    // CPU data
    cpuid::init();

    // Filesystem
    filesystem::init();

    // Keyboard
    driver::keyboard::init();

    // PCI
    driver::pci::init();

    // Disk IO (ATA, IDE, VirtIO)
    interrupt::enable_external_interrupts();
    driver::disk_io::init();
    interrupt::disable_external_interrupts();

    // Memory init late phase
    memory::init_late();

    rreset!();
    rprintln!("Kernel initialized.\n");

    // Load modules
    if let Some(bytes) = filesystem::staticfs::read_file("README.md") {
        let mut lines = 3;
        for b in bytes {
            if b == 0x0a {
                lines -= 1;
                if lines == 0 {
                    break;
                }
            }
            if (0x20 <= b && b <= 0x7f) || b == 0x0a {
                rprint!("{}", b as char);
            }
        }
    } else {
        rprintln!("File not found");
    }

    use crate::multitasking::SCHEDULER;
    let mod_test = multitasking::load_module("mod_test").expect("Module not found");

    for _ in 0..1 {
        let pid = SCHEDULER.try_lock().unwrap().spawn(mod_test);
        rprintln!("Spawned process: pid = {}", pid);
    }

    // Wait until the next clock tick interrupt,
    // after that the process scheduler takes over
    unsafe {
        interrupt::enable_external_interrupts();
        loop {
            asm!("hlt")
        }
    }
}

#[global_allocator]
static HEAP_ALLOCATOR: d7alloc::GlobAlloc = d7alloc::GlobAlloc::new(d7alloc::BumpAllocator::new(
    d7alloc::HEAP_START,
    d7alloc::HEAP_START + d7alloc::HEAP_SIZE,
));

#[alloc_error_handler]
fn out_of_memory(_: Layout) -> ! {
    unsafe {
        asm!("cli"::::"intel","volatile");
        panic_indicator!(0x4f4D4f21); // !M as in "No memory"
        asm!("jmp panic"::::"intel","volatile");
    }
    loop {}
}

#[panic_handler]
#[allow(unused_variables)]
#[no_mangle]
extern "C" fn panic(info: &PanicInfo) -> ! {
    unsafe {
        // asm!("jmp panic"::::"intel","volatile");

        asm!("cli"::::"intel","volatile");
        panic_indicator!(0x4f214f21); // !!

        rforce_unlock!();

        if let Some(location) = info.location() {
            rprintln!(
                "\nKernel Panic: file: '{}', line: {}",
                location.file(),
                location.line()
            );
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

// Static assert assumptions
static_assertions::assert_eq_size!(u64, usize);
