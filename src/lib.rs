// Code style
#![forbid(private_in_public)]
#![forbid(tyvar_behind_raw_pointer)]
#![deny(unused_assignments)]
// Safety
#![deny(overflowing_literals)]
#![deny(safe_packed_borrows)]
#![deny(unused_must_use)]
// Code style (development time)
#![allow(unused_macros)]
#![allow(dead_code)]
// Code style (temp)
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unused_mut)]
#![allow(unreachable_code)]
#![allow(unused_unsafe)]
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
#![feature(box_syntax, box_patterns)]
#![feature(const_fn)]
#![feature(core_intrinsics)]
#![feature(integer_atomics)]
#![feature(lang_items)]
#![feature(maybe_uninit_extra)]
#![feature(naked_functions)]
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
mod initrd;
mod interrupt;
mod ipc;
mod memory;
mod multitasking;
mod services;
mod smp;
mod syscall;
mod syslog;
mod time;

use self::multitasking::SCHEDULER;

/// The kernel main function
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    rreset!();
    rprintln!("Initializing the system...\n");

    syslog::enable();
    driver::pic::init();
    interrupt::init();
    memory::init();
    interrupt::init_after_memory();
    cpuid::init();
    driver::uart::init();
    driver::tsc::init();
    unsafe {
        driver::acpi::init();
        driver::ioapic::init_bsp();
        smp::start_all();
    }
    services::init();

    rreset!();
    log::info!("Kernel initialized.");

    syslog::disable_direct_vga();

    // Start service daemon
    crate::memory::configure(|mut mem_ctrl| {
        let mut sched = SCHEDULER.lock();

        let bytes = crate::initrd::read("serviced").expect("serviced missing from initrd");
        let elfimage = multitasking::process::load_elf(mem_ctrl, bytes);
        sched.spawn(mem_ctrl, elfimage);
    });

    // Hand over to the process scheduler
    multitasking::SCHEDULER_ENABLED.store(true, Ordering::SeqCst);
    unsafe {
        asm!("int 0x30");
    }
    panic!("Returned from the scheduler");
}

/// Used by new AP core for setting up a stack
#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn rust_ap_get_stack() -> u64 {
    smp::ap_take_stack()
}
/// Main function for other cores
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn rust_ap_main() -> ! {
    log::info!("AP core online, getting id...");
    let processor_id = smp::current_processor_id();
    log::info!("AP core {} online", processor_id);

    interrupt::init_smp_ap();
    log::info!("Interrupt handler initialized");

    driver::ioapic::per_processor_init();
    log::info!("APIC initialized");

    smp::ap_mark_ready();
    log::info!("AP core {} ready", processor_id);

    log::trace!("INTO @ {}", self::driver::ioapic::lapic::processor_id());
    loop {
        self::driver::tsc::sleep_ns(1_000_000_000);
        // log::info!("TICK @ {}", self::driver::ioapic::lapic::processor_id());
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
        asm!("cli");
        panic_indicator!(0x4f4D4f21); // !M as in "No memory"
        asm!("jmp panic_stop");
    }
    loop {}
}

use core::sync::atomic::{AtomicBool, Ordering};

/// Tracks wheter a panic is already active, so that
/// if panic printing itself panics, it can be skipped
static PANIC_ACTIVE: AtomicBool = AtomicBool::new(false);

#[panic_handler]
#[cfg(not(test))]
#[allow(unused_variables)]
#[no_mangle]
extern "C" fn panic(info: &PanicInfo) -> ! {
    unsafe {
        bochs_magic_bp!();
        asm!("cli");
        panic_indicator!(0x4f214f21); // !!

        if !PANIC_ACTIVE.load(Ordering::SeqCst) {
            PANIC_ACTIVE.store(true, Ordering::SeqCst);
            panic_indicator!(0x4f234f21); // !#

            if let Some(location) = info.location() {
                log::error!(
                    "Kernel Panic: file: '{}', line: {}",
                    location.file(),
                    location.line()
                );
            } else {
                log::error!("Kernel Panic: Location unavailable");
            }
            if let Some(msg) = info.message() {
                log::error!("  {:?}", msg);
            } else {
                log::error!("  Info unavailable");
            }

            // Stop other cores as well
            driver::ioapic::broadcast_ipi(false, 0xdd);

            asm!("jmp panic_stop");
        } else {
            panic_indicator!(0x4f254f21); // !%
        }
    }
    loop {}
}

// Static assert assumptions
static_assertions::assert_eq_size!(u64, usize);
