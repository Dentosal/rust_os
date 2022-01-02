//! https://wiki.osdev.org/Programmable_Interval_Timer
//! Only used for short-timed sleeps, e.g. for measuring
//! TSC/HPET/APICTimer speed

#![allow(unused_variables)]

use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

const PIT_CH0: u16 = 0x40; // Channel 0 data port (read/write) (PIC TIMER)
const PIT_CH1: u16 = 0x41; // Channel 1 data port (read/write) (UNUSED)
const PIT_CH2: u16 = 0x42; // Channel 2 data port (read/write) (PC SPEAKER)
const PIT_REG: u16 = 0x43; // Mode/Command register (write only, a read is ignored)
const PIT_CH2_CONTROL: u16 = 0x61;

/// Frequency of the internal oscillator, in Hz
const FREQ_HZ: u32 = 1_193_182;

/// Returns the number of nanoseconds between ticks
fn set_freq_and_start(target_freq_hz: u32) -> u64 {
    assert!(target_freq_hz >= 10, "Requested PIT frequency too low");
    assert!(
        target_freq_hz < FREQ_HZ / 2,
        "Requested PIT frequency too high"
    );
    let reload_value = FREQ_HZ / target_freq_hz;
    assert!(reload_value > 0);
    assert!(reload_value <= (u16::MAX as u32));
    unsafe {
        // Channel 0, lobyte/hibyte, Rate generator, Binary mode
        cpuio::outb(0b00_11_010_0, PIT_REG); // command
        cpuio::outb(reload_value as u8, PIT_CH0); // low
        cpuio::outb((reload_value >> 8) as u8, PIT_CH0); // high
    }
    let actual_freq_hz = FREQ_HZ / reload_value;
    const MUL_NSEC: u64 = 1_000_000_000;
    MUL_NSEC / (actual_freq_hz as u64)
}

static ELAPSED_TICKS: AtomicU64 = AtomicU64::new(0);

/// Sleeps specified number of nanoseconds as accurately as possible.
/// Only usable during kernel initialization.
/// Enables and disables exceptions to work.
pub fn kernel_early_sleep_ns(ns: u64) {
    ELAPSED_TICKS.store(0, Ordering::SeqCst);
    let ns_per_tick = set_freq_and_start(1_000);
    let total_ticks = ns / ns_per_tick;
    unsafe {
        asm!("sti");
        while ELAPSED_TICKS.load(Ordering::SeqCst) < total_ticks {
            asm!("xor rax, rax", lateout("rax") _);
            // asm!("hlt");
        }
        asm!("cli");
    }
}

#[inline]
pub fn callback() {
    ELAPSED_TICKS.fetch_add(1, Ordering::SeqCst);
}

/// After the PIT is no longer used, this disables it
/// Will cause a single interrupt as it uses one-shot mode
pub fn disable() {
    log::debug!("Disabling PIT");
    unsafe {
        // Channel 0, lobyte/hibyte, Interrupt On Terminal Count, Binary mode
        cpuio::outb(0b00_11_000_0, PIT_REG); // command
        cpuio::outb(0, PIT_CH0); // low
        cpuio::outb(0, PIT_CH0); // high
    }
}
