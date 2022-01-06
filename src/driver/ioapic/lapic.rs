//! Processor-local APIC

use core::ptr;
use x86_64::VirtAddr;

use crate::driver::acpi::ACPI_DATA;
use crate::memory;
use crate::smp::sleep::lapic_freq_hz;
use crate::smp::ProcessorId;

mod reg {
    #[derive(Debug, Clone, Copy)]
    pub struct LapicReg(u64);
    impl LapicReg {
        pub fn get(self) -> u64 {
            self.0
        }
    }

    pub const LAPIC_ID: LapicReg = LapicReg(0x20);
    pub const LAPIC_VERSION: LapicReg = LapicReg(0x30);
    pub const TASK_PRIORITY: LapicReg = LapicReg(0x80);
    pub const ARBITRATION_PRIORITY: LapicReg = LapicReg(0x90);
    pub const PROCESSOR_PRIORITY: LapicReg = LapicReg(0xa0);
    pub const END_OF_INTERRUPT: LapicReg = LapicReg(0xb0);
    pub const REMOTE_READ: LapicReg = LapicReg(0xc0);
    pub const LOCAL_DESTINATION: LapicReg = LapicReg(0xd0);
    pub const DESTINATION_FORMAT: LapicReg = LapicReg(0xe0);
    pub const SPURIOUS_IV: LapicReg = LapicReg(0xf0);
    pub const ISR_BASE: LapicReg = LapicReg(0x100);
    pub const TRIGGER_MODE_BASE: LapicReg = LapicReg(0x180);
    pub const INTERRUPT_REQUEST: LapicReg = LapicReg(0x200);
    pub const ERROR_STATUS: LapicReg = LapicReg(0x280);
    pub const LVT_CMCI: LapicReg = LapicReg(0x2f0);
    pub const INTERRUPT_COMMAND_BASE: LapicReg = LapicReg(0x300);
    pub const LVT_TIMER: LapicReg = LapicReg(0x320);
    pub const LVT_THERMAL_SENSOR: LapicReg = LapicReg(0x330);
    pub const LVT_PM_COUNTERS: LapicReg = LapicReg(0x340);
    pub const LVT_LINT0: LapicReg = LapicReg(0x350);
    pub const LVT_LINT1: LapicReg = LapicReg(0x360);
    pub const LVT_ERROR: LapicReg = LapicReg(0x370);
    pub const TIMER_INITIAL_COUNT: LapicReg = LapicReg(0x380);
    pub const TIMER_CURRENT_COUNT: LapicReg = LapicReg(0x390);
    pub const TIMER_DIVIDE_CONFIG: LapicReg = LapicReg(0x3e0);
}

const TIMER_MODE_ONE_SHOT: u32 = 0x00000;
const TIMER_MODE_DISABLED: u32 = 0x10000;
const TIMER_MODE_PERIODIC: u32 = 0x20000;
const TIMER_MODE_DEADLINE: u32 = 0x40000;

const TIMER_DIVIDE_BY_1: u32 = 0xb;
const TIMER_DIVIDE_BY_2: u32 = 0x0;
const TIMER_DIVIDE_BY_4: u32 = 0x8;
const TIMER_DIVIDE_BY_8: u32 = 0x2;
const TIMER_DIVIDE_BY_16: u32 = 0xa;
const TIMER_DIVIDE_BY_32: u32 = 0x1;
const TIMER_DIVIDE_BY_64: u32 = 0x9;
const TIMER_DIVIDE_BY_128: u32 = 0x3;

/// Address of the processor-local APIC
pub fn addr() -> VirtAddr {
    let phys_addr = ACPI_DATA
        .poll()
        .expect("acpi::init not called")
        .local_apic_addr;

    memory::phys_to_virt(phys_addr)
}

#[must_use]
pub fn read_u32(offset: reg::LapicReg) -> u32 {
    unsafe { ptr::read_volatile((addr().as_u64() + offset.get()) as *const u32) }
}

pub fn write_u32(offset: reg::LapicReg, value: u32) {
    unsafe { ptr::write_volatile((addr().as_u64() + offset.get()) as *mut u32, value) };
}

/// Get APIC ID of the current CPU
pub fn processor_id() -> ProcessorId {
    ProcessorId((read_u32(reg::LAPIC_ID) >> 24) as u8)
}

/// Tell LAPIC that the interrupt has been processed
pub fn write_eoi() {
    write_u32(reg::END_OF_INTERRUPT, 0);
}

pub fn configure_timer(vector_number: u8) {
    if crate::cpuid::tsc_supports_deadline_mode() {
        // Vector number, TSC-deadline mode, and unmask
        write_u32(reg::LVT_TIMER, (vector_number as u32) | TIMER_MODE_DEADLINE);
        log::trace!("TSC-deadline timer configured");
    } else {
        // Use divider 2, as 1 is buggy on some devices
        write_u32(reg::TIMER_DIVIDE_CONFIG, TIMER_DIVIDE_BY_2);

        //  Vector number, Oneshot mode
        write_u32(reg::LVT_TIMER, (vector_number as u32) | TIMER_MODE_ONE_SHOT);

        log::trace!("TSC-one-shot timer configured");
    }
}

#[inline]
pub fn set_timer_ticks(ticks: u32) {
    assert!(
        !crate::cpuid::tsc_supports_deadline_mode(),
        "Not supported in TSC-deadline mode"
    );
    log::trace!("TSC-one-shot timer ticks {}", ticks);
    log::trace!("TSC-one-shot timer hz {}", lapic_freq_hz());
    set_timer_raw(ticks / 2);
}

#[inline]
pub fn set_timer_raw(ticks: u32) {
    write_u32(reg::TIMER_INITIAL_COUNT, ticks);
}

#[inline]
pub fn get_timer_raw() -> u32 {
    read_u32(reg::TIMER_CURRENT_COUNT)
}
