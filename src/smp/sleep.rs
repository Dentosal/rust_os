use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::time::BSPInstant;

use crate::driver::ioapic::lapic;
use crate::driver::pit;
use crate::driver::tsc;

/// TSC frequency in Hz, measured on `init`.
/// As the kernel uses invariant TSC, the tick rate is constant.
static TSC_FREQ_HZ: AtomicU64 = AtomicU64::new(0);

/// LAPIC tick frequency in Hz, measured on `init`.
static LAPIC_FREQ_HZ: AtomicU64 = AtomicU64::new(0);

#[inline]
pub fn freq_hz() -> u64 {
    if crate::cpuid::tsc_supports_deadline_mode() {
        tsc_freq_hz()
    } else {
        lapic_freq_hz()
    }
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

#[inline]
pub fn tsc_freq_hz() -> u64 {
    let value = TSC_FREQ_HZ.load(Ordering::SeqCst);
    assert!(value != 0, "TSC_FREQ_HZ uninitialized");
    value
}

#[inline]
pub fn lapic_freq_hz() -> u64 {
    let value = LAPIC_FREQ_HZ.load(Ordering::SeqCst);
    assert!(value != 0, "LAPIC_FREQ_HZ uninitialized");
    value
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlreadyExpired;

#[must_use]
pub fn set_deadline(instant: BSPInstant) -> Result<(), AlreadyExpired> {
    let now = BSPInstant::now();
    log::trace!("Setting sleep deadline to {:?} (now={:?}", instant, now);
    if crate::cpuid::tsc_supports_deadline_mode() {
        tsc::set_deadline(instant.tsc_value());
        Ok(()) // TODO
    } else {
        let ticks = instant.try_ticks_from(now).unwrap_or(10) as u32;
        lapic::set_timer_ticks(ticks);
        if ticks == 0 {
            Err(AlreadyExpired)
        } else {
            Ok(())
        }
    }
}

pub fn clear_deadline() {
    if crate::cpuid::tsc_supports_deadline_mode() {
        tsc::clear_deadline();
    } else {
        lapic::set_timer_ticks(0);
    }
}

/// Interrupts must be disabled before calling this
pub fn sleep_until(deadline: BSPInstant) {
    let r = set_deadline(deadline);
    if r == Err(AlreadyExpired) {
        return;
    }
    unsafe {
        while BSPInstant::now() < deadline {
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

/// Interrupts must be disabled before calling this
pub fn sleep_ns(ns: u64) {
    sleep_until(BSPInstant::now().add_ticks(crate::smp::sleep::ns_to_ticks(ns)));
}

fn measure_with_pit() {
    // TSC
    let t0 = tsc::read();
    pit::kernel_early_sleep_ns(100_000_000);
    let t1 = tsc::read();

    // LAPIC timer
    lapic::set_timer_raw(0xffff_ffff);
    pit::kernel_early_sleep_ns(100_000_000);
    let after = lapic::get_timer_raw();
    let tick_count = 0xffff_ffff - after;

    pit::disable();

    let tsc_freq_hz = 10 * (t1 - t0);
    TSC_FREQ_HZ.store(tsc_freq_hz, Ordering::SeqCst);
    log::info!("TSC frequency Hz {}", tsc_freq_hz);

    let lapic_freq_hz = 10 * (tick_count as u64);
    LAPIC_FREQ_HZ.store(lapic_freq_hz, Ordering::SeqCst);
    log::info!("LAPIC frequency Hz {}", lapic_freq_hz);
}

pub fn init() {
    measure_with_pit();
}
