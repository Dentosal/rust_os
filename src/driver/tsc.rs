//! Intel guarantees that TSC will not overflow within 10 years of last
//! CPU reset (or counter reset).

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// TSC frequency in Hz, measured on `init`.
/// As the kernel uses invariant TSC, the tick rate is constant.
static TSC_FREQ_HZ: AtomicU64 = AtomicU64::new(0);

#[inline]
pub fn freq_hz() -> u64 {
    let value = TSC_FREQ_HZ.load(Ordering::SeqCst);
    assert!(value != 0, "TSC_FREQ_HZ uninitialized");
    value
}

/// Convert nanoseconds to TSC ticks
pub fn ns_to_ticks(ns: u64) -> u64 {
    // Limit tick counts to one year
    assert!(
        ns < 365 * 24 * 60 * 60 * 1_000_000_000,
        "Deadlines beyond one year are not allowed"
    );
    // Do calculation differently depending on value size.
    // This avoids overflow in all cases, and improves accuracy.

    if ns > 1_000_000_000 {
        // Sleep is over one second, millisecond-accuracy
        let offset_ms = ns / 1_000_000;
        let freq_khz = freq_hz() / 1_000;
        offset_ms * freq_khz
    } else if ns > 1_000_000 {
        // Sleep is over 1ms, microsecond-accuracy
        let offset_us = ns / 1_000;
        (offset_us * freq_hz()) / 1_000_000
    } else {
        // Sleep is measured in microseconds, full accuracy
        (ns * freq_hz()) / 1_000_000_000
    }
}

/// Convert TSC ticks to nanoseconds
pub fn ticks_to_ns(ticks: u64) -> u64 {
    // TODO: improve accuracy by splitting like in `ns_to_ticks`
    assert!(ticks < u64::MAX / 1_000_000);
    (ticks * 1_000_000) / (freq_hz() / 1_000)
}

fn measure_with_pit() {
    let t0 = read();
    super::pit::kernel_early_sleep_ns(10_000_000);
    let t1 = read();

    super::pit::disable();

    let tsc_freq_hz = 100 * (t1 - t0);
    TSC_FREQ_HZ.store(tsc_freq_hz, Ordering::SeqCst);
    log::info!("TSC frequency Hz {}", tsc_freq_hz);
}

pub fn init() {
    measure_with_pit();
}

/// Reset TSC to zero.
/// This should be used between deadlines if TSC value is near overflow.
/// # Warning
/// Do not reset the counter if a deadline is currently being used.
#[inline]
pub fn reset() {
    unsafe {
        asm!("wrmsr",
            in("ecx") 0x10, // TSC MSR
            in("edx") 0u32,
            in("eax") 0u32,
            options(nostack, nomem)
        )
    }
}

/// Read TSC value, serializing
#[inline]
pub fn read() -> u64 {
    let rdx: u64;
    let rax: u64;
    unsafe {
        asm!(
            "rdtscp", // Serializing read
            out("rdx") rdx,
            out("rax") rax,
            out("rcx") _,
            options(nomem, nostack)
        )
    }

    (rdx << 32) | (rax & 0xffff_ffff)
}

/// Sets deadline
#[inline]
fn set_deadline(deadline: u64) {
    // log::trace!("Set deadline {} (current {})", deadline, read());
    unsafe {
        asm!("wrmsr",
            in("ecx") 0x6e0,
            in("edx") (deadline >> 32) as u32,
            in("eax") deadline as u32,
            options(nostack, nomem)
        )
    }
}

/// Cancels deadline and disarms timer
#[inline]
fn clear_deadline() {
    unsafe {
        asm!("wrmsr",
            in("ecx") 0x6e0,
            in("edx") 0u32,
            in("eax") 0u32,
            options(nostack, nomem)
        )
    }
}

/// Interrupts must be disabled before calling this
pub fn sleep_until(deadline: u64) {
    set_deadline(deadline);
    unsafe {
        while read() < deadline {
            // This hlt is executed before the first interrupt is processed.
            // Other exceptions are processed during the sleep as well.
            // The processor wakes up from hlt on every interrupt, and
            // TSC is checked after that to see if it was the TSC deadline.
            // Therefore this loop doesn't require any explicit synchronization.
            asm!("sti; hlt");
        }
        asm!("cli");
    }
}

pub fn sleep_ticks(ticks: u64) {
    sleep_until(read() + ticks);
}

pub fn sleep_ns(ns: u64) {
    sleep_ticks(ns_to_ticks(ns));
}

pub fn set_deadline_ticks(ticks: u64) {
    set_deadline(read() + ticks);
}

pub fn set_deadline_ns(ns: u64) {
    set_deadline_ticks(ns_to_ticks(ns));
}
