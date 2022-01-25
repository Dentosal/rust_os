//! https://wiki.osdev.org/IOAPIC#Programming_the_I.2FO_APIC

use core::{mem, ptr};

use super::super::acpi::tables::madt::entry::IoAPIC as MadtEntry;
use super::ACPI_DATA;
use crate::memory;

const REG_ID: u32 = 0;
const REG_VERSION: u32 = 1;
const REG_ARBITRATION_ID: u32 = 2;

/// Redirect table entry
/// https://wiki.osdev.org/IOAPIC#IOREDTBL
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct RedirectEntry {
    vector: u8,
    flags: RedirectEntryFlags,
    masked: bool,
    reserved: [u8; 4],
    destination: u8,
}

impl RedirectEntry {
    fn new(vector: u8, flags: RedirectEntryFlags, masked: bool, destination: u8) -> Self {
        Self {
            vector,
            flags,
            masked,
            reserved: [0; 4],
            destination,
        }
    }
}

static_assertions::assert_eq_size!(RedirectEntry, u64);

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct RedirectEntryFlags(u8);

impl RedirectEntryFlags {
    fn new(
        delivery_mode: DeliveryMode, destination_logical: bool, pending: bool,
        pin_polarity_low: bool, remote_irr: bool, _trigger_mode_level: bool,
    ) -> Self {
        Self(
            (delivery_mode as u8)
                | ((destination_logical as u8) << 4)
                | ((pending as u8) << 5)
                | ((pin_polarity_low as u8) << 6)
                | ((remote_irr as u8) << 7),
        )
    }
}

/// How the interrupt will be sent to the CPU(s)
#[derive(Debug, Copy, Clone)]
enum DeliveryMode {
    Fixed = 0,
    LowPriority = 1,
    SMI = 2,
    NMI = 4,
    Init = 5,
    ExternalInt = 7,
}

fn actual_addr(entry: &MadtEntry, offset: u32) -> *mut u32 {
    let phys = memory::prelude::PhysAddr::new((entry.io_acpi_addr + offset) as u64);
    memory::phys_to_virt(phys).as_mut_ptr()
}

/// Select active register
fn select(entry: &MadtEntry, offset: u32) {
    unsafe {
        ptr::write_volatile(actual_addr(entry, 0), offset);
    }
}

/// Read from a register
fn read(entry: &MadtEntry, offset: u32) -> u32 {
    select(entry, offset);
    unsafe { ptr::read_volatile(actual_addr(entry, 0x10)) }
}

/// Write to a register
fn write(entry: &MadtEntry, offset: u32, value: u32) {
    select(entry, offset);
    unsafe { ptr::write_volatile(actual_addr(entry, 0x10), value) }
}

/// APIC id
fn id(entry: &MadtEntry) -> u8 {
    ((read(entry, REG_ID) >> 24) & 0xf) as u8
}

/// Version number and max entry
fn version(entry: &MadtEntry) -> (u8, u8) {
    let v = read(entry, REG_VERSION);
    (v as u8, (v >> 16) as u8)
}

/// # Safety
/// Arg relative_irq MUST be <= max redirection entry (returned by `version().1`)
unsafe fn read_redirect_entry(entry: &MadtEntry, relative_irq: u8) -> RedirectEntry {
    let req = 0x10 + (relative_irq as u32) * 2;

    let lo = read(entry, req);
    let hi = read(entry, req + 1);
    let val = (lo as u64) | ((hi as u64) << 32);
    unsafe { mem::transmute(val) }
}

/// # Safety
/// Arg relative_irq MUST be <= max redirection entry (returned by `version().1`)
unsafe fn write_redirect_entry(entry: &MadtEntry, relative_irq: u8, redirect: RedirectEntry) {
    let req = 0x10 + (relative_irq as u32) * 2;

    let bits: u64 = unsafe { mem::transmute(redirect) };

    write(entry, req, bits as u32);
    write(entry, req + 1, (bits >> 32) as u32);
}

fn set_irq_handler(io_apics: &[MadtEntry], irq: u8, redirect: RedirectEntry) {
    for apic in io_apics {
        // Test if this apic is handling the given irq
        if (irq as u32) >= apic.gsib {
            let relative_irq = (irq as u32) - apic.gsib;
            let max = version(apic).1 as u32;
            if relative_irq <= max {
                // This is the right I/O APIC
                unsafe {
                    write_redirect_entry(apic, relative_irq as u8, redirect);
                }
                return;
            }
        }
    }
    panic!("No I/O APIC handles irq {}", irq);
}

pub fn init() {
    let acpi_data = ACPI_DATA.poll().expect("acpi::init not called");

    let handling_cpu_id = acpi_data.cpus[0].acpi_id;
    let io_apics = &acpi_data.io_apics;

    if io_apics.is_empty() {
        panic!("No I/O APICs detected, unsupported system");
    }

    // TODO: actual limit might not be 24
    for src_irq in 0..24 {
        let mut irq = src_irq;
        let mut pin_polarity_low = false;
        let mut trigger_mode_level = false;

        for source_override in &acpi_data.int_source_overrides {
            if source_override.bus_source == 0 && source_override.irq_source == src_irq {
                irq = source_override.gsi as u8;
                pin_polarity_low = source_override.flags & 2 != 0;
                trigger_mode_level = source_override.flags & 8 != 0;
            }
        }

        log::debug!("Mapping I/O apic irq {:#02x} to {:#02x}", src_irq, irq);
        set_irq_handler(
            &io_apics,
            src_irq,
            RedirectEntry::new(
                0x30 + irq,
                RedirectEntryFlags::new(
                    DeliveryMode::Fixed,
                    false,
                    false,
                    pin_polarity_low,
                    false,
                    trigger_mode_level,
                ),
                false, // !enabled.contains(&irq),
                handling_cpu_id,
            ),
        );
    }
}
