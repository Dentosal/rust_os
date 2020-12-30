// Code style
#![forbid(private_in_public)]
#![forbid(tyvar_behind_raw_pointer)]
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
mod random;
mod services;
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
    driver::pic::init();
    interrupt::init();
    memory::init();
    interrupt::init_after_memory();
    driver::pit::init();
    cpuid::init();
    services::init();

    rreset!();
    log::info!("Kernel initialized.");

    syslog::disable_direct_vga();

    // Start service daemon
    crate::memory::configure(|mut mem_ctrl| {
        use crate::multitasking::SCHEDULER;
        let mut sched = SCHEDULER.lock();

        let bytes = crate::initrd::read("serviced").expect("serviced missing from initrd");
        let elfimage = multitasking::process::load_elf(mem_ctrl, bytes);
        sched.spawn(mem_ctrl, elfimage);
        log::trace!("{}", sched.debug_view_string());
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
        asm!("jmp panic_stop"::::"intel","volatile");
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
        asm!("cli"::::"intel","volatile");
        panic_indicator!(0x4f214f21); // !!

        if !PANIC_ACTIVE.load(Ordering::SeqCst) {
            PANIC_ACTIVE.store(true, Ordering::SeqCst);
            panic_indicator!(0x4f234f21); // !#

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
            asm!("jmp panic_stop"::::"intel","volatile");
        } else {
            panic_indicator!(0x4f254f21); // !%
        }
    }
    loop {}
}

// Static assert assumptions
static_assertions::assert_eq_size!(u64, usize);
