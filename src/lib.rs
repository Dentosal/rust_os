// Code style
#![forbid(private_in_public)]
#![forbid(tyvar_behind_raw_pointer)]
#![deny(unused_assignments)]
// Safety
#![deny(overflowing_literals)]
#![deny(safe_packed_borrows)]
#![deny(unused_must_use)]
#![allow(incomplete_features)]
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
// Disable some clippy lints
#![allow(clippy::cast_ptr_alignment)]
#![allow(clippy::empty_loop)]
#![allow(clippy::identity_op)]
#![allow(clippy::inconsistent_digit_grouping)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::unreadable_literal)]
// No-std when not running tests
#![cfg_attr(not(test), no_std)]
// Unstable features
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![feature(asm)]
#![feature(box_into_raw_non_null)]
#![feature(box_syntax, box_patterns)]
#![feature(const_fn)]
#![feature(const_generics)]
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

#[macro_use]
extern crate bitflags;

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
mod memory;
mod multitasking;
mod syscall;
mod syslog;
mod time;

/// The kernel main function
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    rreset!();
    rprintln!("Initializing the system...\n");

    syslog::enable();

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

    // StaticFS requires disk drivers for now
    filesystem::staticfs::init();

    // Memory init late phase
    memory::init_late();

    // NICs
    driver::nic::init();

    rreset!();
    log::info!("Kernel initialized.\n");

    {
        use crate::filesystem::FILESYSTEM;
        let readme_bytes = FILESYSTEM
            .lock()
            .read_file("/mnt/staticfs/README.md")
            .unwrap();
        let mut lines = 3;
        for b in readme_bytes {
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
    }
    rprintln!("");

    crate::memory::configure(|mut mem_ctrl| {
        use crate::filesystem::FILESYSTEM;
        use crate::multitasking::SCHEDULER;

        let mut fs = FILESYSTEM.lock();
        let mut sched = SCHEDULER.lock();

        fs.kernel_exec(&mut mem_ctrl, &mut sched, "/mnt/staticfs/mod_test")
            .expect("Could not spawn");
    });

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
#[cfg(not(test))]
static HEAP_ALLOCATOR: d7alloc::GlobAlloc = d7alloc::GlobAlloc::new(d7alloc::BumpAllocator::new(
    d7alloc::HEAP_START,
    d7alloc::HEAP_START + d7alloc::HEAP_SIZE,
));

#[alloc_error_handler]
#[cfg(not(test))]
fn out_of_memory(_: Layout) -> ! {
    unsafe {
        asm!("cli"::::"intel","volatile");
        panic_indicator!(0x4f4D4f21); // !M as in "No memory"
        asm!("jmp panic"::::"intel","volatile");
    }
    loop {}
}

#[panic_handler]
#[cfg(not(test))]
#[allow(unused_variables)]
#[no_mangle]
extern "C" fn panic(info: &PanicInfo) -> ! {
    unsafe {
        // asm!("jmp panic"::::"intel","volatile");

        asm!("cli"::::"intel","volatile");
        panic_indicator!(0x4f214f21); // !!

        rforce_unlock!();

        if let Some(location) = info.location() {
            log::error!(
                "\nKernel Panic: file: '{}', line: {}",
                location.file(),
                location.line()
            );
        } else {
            log::error!("\nKernel Panic: Location unavailable");
        }
        if let Some(msg) = info.message() {
            log::error!("  {:?}", msg);
        } else {
            log::error!("  Info unavailable");
        }
        asm!("jmp panic"::::"intel","volatile");
    }
    loop {}
}

// Static assert assumptions
static_assertions::assert_eq_size!(u64, usize);
