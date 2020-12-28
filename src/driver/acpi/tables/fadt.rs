use alloc::boxed::Box;
use core::mem;
use core::slice;
use x86_64::PhysAddr;

use crate::memory;

use super::{is_legacy_acpi, rsdt_get, SDTHeader};

// TODO: rename fields to snake_case
#[allow(non_snake_case)]
#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
struct Fadt {
    header: SDTHeader,
    firmware_ctrl: u32,
    dsdt: u32,
    /// field used in ACPI 1.0; no longer in use, for compatibility only
    reserved: u8,
    PreferredPowerManagementProfile: u8,
    SCI_Interrupt: u16,
    SMI_CommandPort: u32,
    AcpiEnable: u8,
    AcpiDisable: u8,
    S4BIOS_REQ: u8,
    PSTATE_Control: u8,
    PM1aEventBlock: u32,
    PM1bEventBlock: u32,
    PM1aControlBlock: u32,
    PM1bControlBlock: u32,
    PM2ControlBlock: u32,
    PMTimerBlock: u32,
    GPE0Block: u32,
    GPE1Block: u32,
    PM1EventLength: u8,
    PM1ControlLength: u8,
    PM2ControlLength: u8,
    PMTimerLength: u8,
    GPE0Length: u8,
    GPE1Length: u8,
    GPE1Base: u8,
    CStateControl: u8,
    WorstC2Latency: u16,
    WorstC3Latency: u16,
    FlushSize: u16,
    FlushStride: u16,
    DutyOffset: u8,
    DutyWidth: u8,
    DayAlarm: u8,
    MonthAlarm: u8,
    Century: u8,
    /// reserved in ACPI 1.0; used since ACPI 2.0+
    BootArchitectureFlags: u16,
    reserved2: u8,
    Flags: u32,
    ResetReg: GenericAddress,
    ResetValue: u8,
    reserved3: [u8; 3],
    /// 64bit pointers - Available on ACPI 2.0+
    X_FirmwareControl: u64,
    x_dsdt: u64,
    X_PM1aEventBlock: GenericAddress,
    X_PM1bEventBlock: GenericAddress,
    X_PM1aControlBlock: GenericAddress,
    X_PM1bControlBlock: GenericAddress,
    X_PM2ControlBlock: GenericAddress,
    X_PMTimerBlock: GenericAddress,
    X_GPE0Block: GenericAddress,
    X_GPE1Block: GenericAddress,
}

/// https://wiki.osdev.org/FADT#GenericAddressStructure
#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
struct GenericAddress {
    address_space: u8,
    bit_width: u8,
    bit_offset: u8,
    access_size: u8,
    address: u64,
}

pub fn init() {
    let fadt_ptr = rsdt_get(b"FACP").expect("FADT not found");
    let addr = memory::phys_to_virt(fadt_ptr);
    let fadt: Fadt = unsafe { *addr.as_ptr() };

    let legacy = is_legacy_acpi();
    let dsdt_phys = if legacy {
        fadt.dsdt as u64
    } else {
        fadt.x_dsdt
    };

    let vptr = memory::phys_to_virt(PhysAddr::new(dsdt_phys));
    let dsdt_header: SDTHeader = unsafe { *vptr.as_ptr() };

    let size = dsdt_header.length as usize;
    let _code: &[u8] = unsafe { slice::from_raw_parts(vptr.as_ptr(), size) };

    // TODO: _code is AML code, and should be given to an interpreter
}
