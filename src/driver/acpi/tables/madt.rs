use alloc::vec::Vec;
use core::mem;
use core::ptr;
use core::sync::atomic::AtomicU32;
use x86_64::PhysAddr;

use crate::memory;

use super::SDTHeader;

/// https://wiki.osdev.org/MADT
#[allow(non_camel_case_types)]
pub mod entry {
    #[derive(Debug, Copy, Clone)]
    #[repr(C, packed)]
    pub struct ProcessorLocalAPIC {
        pub processor_id: u8,
        pub acpi_id: u8,
        flags: u32,
    }
    #[derive(Debug, Copy, Clone)]
    #[repr(C, packed)]
    pub struct IoAPIC {
        pub io_acpi_id: u8,
        reserved: u8,
        /// MMIO base address
        pub io_acpi_addr: u32,
        /// Global system interrupt (first IRQ handled by this)
        pub gsib: u32,
    }
    #[derive(Debug, Copy, Clone)]
    #[repr(C, packed)]
    pub struct InterruptSourceOverride {
        pub bus_source: u8,
        pub irq_source: u8,
        pub gsi: u32,
        pub flags: u16,
    }
    #[derive(Debug, Copy, Clone)]
    #[repr(C, packed)]
    pub struct NonMaskableInterrupts {
        /// 0xff for all processors
        processor_id: u8,
        flags: u16,
        lint: u8,
    }
    #[derive(Debug, Copy, Clone)]
    #[repr(C, packed)]
    pub struct LocalAPICAddressOverride {
        reserved: u16,
        local_apic_addr: u64,
    }
}

pub struct ACPIData {
    pub local_apic_addr: PhysAddr,
    pub cpus: Vec<entry::ProcessorLocalAPIC>,
    pub io_apics: Vec<entry::IoAPIC>,
    pub int_source_overrides: Vec<entry::InterruptSourceOverride>,
}

pub static ACPI_DATA: spin::Once<ACPIData> = spin::Once::new();

pub fn init() {
    let apic_addr = super::rsdt_get(b"APIC").expect("APIC table not found");
    let virt_addr = memory::phys_to_virt(apic_addr);

    let header: SDTHeader = unsafe { *virt_addr.as_ptr() };
    let h_size = mem::size_of::<SDTHeader>() as u64;

    assert!(header.length > 8);

    let mut ptr = virt_addr.as_u64() + h_size;

    let local_apic_addr_u32: u32 = unsafe { *(ptr as *const _) };
    ptr += 4;
    let _flags: u32 = unsafe { *(ptr as *const _) };
    ptr += 4;

    let local_apic_addr = PhysAddr::new(local_apic_addr_u32 as u64);
    let mut cpus = Vec::new();
    let mut io_apics = Vec::new();
    let mut int_source_overrides = Vec::new();

    let table_end = virt_addr.as_u64() + header.length as u64;

    loop {
        let entry_type: u8 = unsafe { *(ptr as *const _) };
        ptr += 1;
        let entry_size: u8 = unsafe { *(ptr as *const _) };
        ptr += 1;

        let entry_body_size = (entry_size - 2) as usize;

        match entry_type {
            0 => {
                assert_eq!(entry_body_size, mem::size_of::<entry::ProcessorLocalAPIC>());
                let entry: entry::ProcessorLocalAPIC = unsafe { *(ptr as *const _) };
                log::debug!("{:#?}", entry);
                cpus.push(entry);
            },
            1 => {
                assert_eq!(entry_body_size, mem::size_of::<entry::IoAPIC>());
                let entry: entry::IoAPIC = unsafe { *(ptr as *const _) };
                log::debug!("{:#?}", entry);
                io_apics.push(entry);
            },
            2 => {
                assert_eq!(
                    entry_body_size,
                    mem::size_of::<entry::InterruptSourceOverride>()
                );
                let entry: entry::InterruptSourceOverride = unsafe { *(ptr as *const _) };
                log::debug!("{:#?}", entry);
                int_source_overrides.push(entry);
            },
            4 => {
                assert_eq!(
                    entry_body_size,
                    mem::size_of::<entry::NonMaskableInterrupts>()
                );
                let entry: entry::NonMaskableInterrupts = unsafe { *(ptr as *const _) };
                log::debug!("{:#?}", entry);
            },
            5 => {
                assert_eq!(
                    entry_body_size,
                    mem::size_of::<entry::LocalAPICAddressOverride>()
                );
                let entry: entry::LocalAPICAddressOverride = unsafe { *(ptr as *const _) };
                log::debug!("{:#?}", entry);
            },
            other => panic!("Unknown APIC entry type {}", other),
        }

        ptr += entry_body_size as u64;

        if ptr == table_end {
            break;
        }

        assert!(ptr < table_end);
    }

    ACPI_DATA.call_once(|| ACPIData {
        local_apic_addr,
        cpus: cpus.clone(),
        io_apics,
        int_source_overrides,
    });
}
