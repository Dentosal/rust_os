// Code style
#![forbid(private_in_public)]
#![forbid(tyvar_behind_raw_pointer)]
// Safety
#![deny(overflowing_literals)]
#![deny(unused_must_use)]
// Workarounds
#![allow(named_asm_labels)]
// Code style (development time)
#![allow(unused_macros)]
#![allow(dead_code)]
// Code style (temp)
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
// No stdlib or mainfn when not running tests
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
// Unstable features
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(allocator_api)]
#![feature(asm_const)]
#![feature(box_syntax, box_patterns)]
#![feature(core_intrinsics)]
#![feature(integer_atomics)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(panic_info_message)]
#![feature(ptr_internals, ptr_metadata)]
#![feature(stmt_expr_attributes)]
#![feature(trait_alias)]
#![feature(inline_const)]
#![feature(drain_filter)]
#![feature(int_roundings)]

use core::alloc::Layout;
use core::arch::asm;
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
mod signature;
mod smp;
mod syscall;
mod syslog;
mod time;

use self::multitasking::SCHEDULER;

/// The kernel main function
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    rreset!();
    rprintln!("Initializing the system...\n");

    driver::uart::init();
    syslog::enable();
    unsafe {
        driver::pic::init();
        interrupt::init();
        memory::init();
        interrupt::init_after_memory();
        cpuid::init();
        random::init();
        driver::acpi::init();
        smp::init();
        driver::ioapic::init_bsp();
        // smp::start_all();
    }
    services::init();

    #[cfg(feature = "self-test")]
    {
        log::info!("Self-test successful");
        driver::acpi::power_off();
    }

    rreset!();
    log::info!("Kernel initialized.");

    syslog::disable_direct_vga();

    // Start service daemon
    {
        let bytes = crate::initrd::read("serviced").expect("serviced missing from initrd");
        let elfimage = multitasking::load_elf(bytes).expect("Could not load image");

        let mut sched = SCHEDULER.try_lock().unwrap();
        sched.spawn(&[], elfimage).unwrap();
    }

    // Hand over to the process scheduler
    multitasking::SCHEDULER_ENABLED.store(true, Ordering::SeqCst);
    unsafe {
        asm!("int 0xd8");
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
        crate::smp::sleep::sleep_ns(1_000_000_000);
        // log::info!("TICK @ {}", self::driver::ioapic::lapic::processor_id());
    }
}

#[global_allocator]
#[cfg(not(test))]
static HEAP_ALLOCATOR: memory::rust_heap::GlobAlloc = memory::rust_heap::GlobAlloc::new();

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

core::arch::global_asm!(
    "
.global panic_stop
.section .text

panic_stop:
    mov rax, 0x4f214f214f214f21 // !!!!
    mov [0xb8000], rax
    cli
    hlt
    .lp: jmp .lp
"
);

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

            // Attempt to print backtrace as well
            // stack_trace();

            // Stop other cores as well
            driver::ioapic::broadcast_ipi(false, 0xdd);

            asm!("jmp panic_stop");
        } else {
            panic_indicator!(0x4f254f21); // !%
        }
    }
    loop {}
}

#[inline(never)]
pub unsafe fn stack_trace() {
    let mut rbp: usize;
    core::arch::asm!("mov {}, rbp", out(reg) rbp);

    log::error!("TRACE: {:>016X}", rbp);
    //Maximum 64 frames
    for _frame in 0..64 {
        if let Some(rip_rbp) = rbp.checked_add(core::mem::size_of::<usize>()) {
            if let Ok(_rbp_virt) = x86_64::VirtAddr::try_new(rbp as u64) {
                if let Ok(_rip_rbp_virt) = x86_64::VirtAddr::try_new(rip_rbp as u64) {
                    // TODO: check that rbp, rip_rbp are canonical and map to an addr
                    let rip = *(rip_rbp as *const usize);
                    if rip == 0 {
                        log::error!(" {:>016X}: EMPTY RETURN", rbp);
                        break;
                    }
                    log::error!("  {:>016X}: {:>016X}", rbp, rip);
                    rbp = *(rbp as *const usize);
                    // TODO: resolve symbol by rip if the symbol map is available
                } else {
                    log::error!("  {:>016X}: GP", rip_rbp);
                    break;
                }
            } else {
                log::error!("  {:>016X}: GUARD PAGE", rbp);
                break;
            }
        }
    }
    log::error!("TRACE OVER");
}

// Static assert assumptions
static_assertions::assert_eq_size!(u64, usize);
