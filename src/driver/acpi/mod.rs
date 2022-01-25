use core::arch::asm;
use core::mem;

use spin::{Mutex, Once};
use x86_64::PhysAddr;

use acpi::{
    fadt::Fadt, platform::address::GenericAddress, sdt::Signature, AcpiHandler, AcpiTables,
    PhysicalMapping,
};
use aml::{value::Args, AmlValue};

pub mod tables;

use crate::memory::phys_to_virt;

pub use self::tables::madt::ACPI_DATA;

/// Addtional info: http://wiki.osdev.org/RSDP
pub unsafe fn get_rsdp_addr() -> Option<PhysAddr> {
    const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";

    // TODO: Check hard-coded points before looping, at least: 0x000fa6a0 (bochs)

    // Scan EBDA
    // TODO: use actual value from BDA (must be saved just after boot) instead of 0x9fc00
    let ebda_start = 0x9fc00; // HACK
    let ebda_end = ebda_start + 0x10 * 0x64;
    for p in (ebda_start..ebda_end).step_by(0x10) {
        // Is this an ACPI structure (quick signature check)
        if &*(p as *const [u8; 8]) == RSDP_SIGNATURE {
            return Some(PhysAddr::new(p as u64));
        }
    }

    // Scan from 0x000E0000 to 0x000FFFFF
    let area_start = 0xe0000;
    let area_end = 0xfffff;
    for p in (area_start..area_end).step_by(0x10) {
        // Is this an ACPI structure (quick signature check)
        if &*(p as *const [u8; 8]) == RSDP_SIGNATURE {
            return Some(PhysAddr::new(p as u64));
        }
    }

    None
}

#[derive(Debug, Clone, Copy)]
struct Handler;

impl AcpiHandler for Handler {
    unsafe fn map_physical_region<T>(
        &self, physical_address: usize, size: usize,
    ) -> PhysicalMapping<Self, T> {
        let addr = PhysAddr::new(physical_address as u64);
        let virt = phys_to_virt(addr);
        PhysicalMapping::new(
            addr.as_u64() as usize,
            core::ptr::NonNull::new_unchecked(virt.as_mut_ptr()),
            size,
            size,
            *self,
        )
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}

#[derive(Debug, Clone, Copy)]
struct AmlHandler;

impl aml::Handler for AmlHandler {
    fn read_u8(&self, address: usize) -> u8 {
        unsafe { *phys_to_virt(PhysAddr::new(address as u64)).as_ptr() }
    }
    fn read_u16(&self, address: usize) -> u16 {
        unsafe { *phys_to_virt(PhysAddr::new(address as u64)).as_ptr() }
    }
    fn read_u32(&self, address: usize) -> u32 {
        unsafe { *phys_to_virt(PhysAddr::new(address as u64)).as_ptr() }
    }
    fn read_u64(&self, address: usize) -> u64 {
        unsafe { *phys_to_virt(PhysAddr::new(address as u64)).as_ptr() }
    }

    fn write_u8(&mut self, address: usize, value: u8) {
        unsafe { *phys_to_virt(PhysAddr::new(address as u64)).as_mut_ptr() = value }
    }
    fn write_u16(&mut self, address: usize, value: u16) {
        unsafe { *phys_to_virt(PhysAddr::new(address as u64)).as_mut_ptr() = value }
    }
    fn write_u32(&mut self, address: usize, value: u32) {
        unsafe { *phys_to_virt(PhysAddr::new(address as u64)).as_mut_ptr() = value }
    }
    fn write_u64(&mut self, address: usize, value: u64) {
        unsafe { *phys_to_virt(PhysAddr::new(address as u64)).as_mut_ptr() = value }
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe { cpuio::UnsafePort::new(port).read() }
    }
    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe { cpuio::UnsafePort::new(port).read() }
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe { cpuio::UnsafePort::new(port).read() }
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe { cpuio::UnsafePort::new(port).write(value) }
    }
    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe { cpuio::UnsafePort::new(port).write(value) }
    }
    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe { cpuio::UnsafePort::new(port).write(value) }
    }

    fn read_pci_u8(&self, _segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u8 {
        let base_offset = offset - offset % 4;
        let base_value = unsafe { pci_read_u32(bus, device, function, base_offset as u8) };
        let shift = 24 - 8 * (offset % 4);
        (base_value >> shift) as u8
    }
    fn read_pci_u16(&self, _segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u16 {
        assert!(offset % 2 == 0);
        let base_offset = offset - offset % 4;
        let base_value = unsafe { pci_read_u32(bus, device, function, base_offset as u8) };
        let shift = 16 - 8 * (offset % 4);
        (base_value >> shift) as u16
    }
    fn read_pci_u32(&self, _segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u32 {
        unsafe { pci_read_u32(bus, device, function, offset as u8) }
    }

    fn write_pci_u8(
        &self, _segment: u16, _bus: u8, _device: u8, _function: u8, _offset: u16, _value: u8,
    ) {
        todo!("write_pci_u8")
    }
    fn write_pci_u16(
        &self, _segment: u16, _bus: u8, _device: u8, _function: u8, _offset: u16, _value: u16,
    ) {
        todo!("write_pci_u16")
    }
    fn write_pci_u32(
        &self, _segment: u16, _bus: u8, _device: u8, _function: u8, _offset: u16, _value: u32,
    ) {
        todo!("write_pci_u32")
    }
}
pub const CONFIG_ADDR: usize = 0xCF8;
pub const CONFIG_DATA: usize = 0xCFC;

unsafe fn pci_read_u32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    assert!(offset % 4 == 0, "offset must be 4-byte aligned");

    let address: u32 = (((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | (offset as u32)
        | (0x80000000u32)) as u32;

    /* write out the address */
    asm!("out dx, eax", in("dx") CONFIG_ADDR, in("eax") address, options(nostack, nomem));
    let inp: u32;
    asm!("in eax, dx", in("dx") CONFIG_DATA, out("eax") inp, options(nostack, nomem));
    inp
}

unsafe fn pci_write_u32(bus: u8, slot: u8, func: u8, offset: u8, value: u32) {
    assert!(offset % 4 == 0, "offset must be 4-byte aligned");

    let address: u32 = (((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | (offset as u32)
        | (0x80000000u32)) as u32;

    /* write out the address */
    asm!("out dx, eax", in("dx") CONFIG_ADDR, in("eax") address, options(nostack, nomem));
    asm!("out dx, eax", in("dx") CONFIG_DATA, in("eax") value, options(nostack, nomem));
}

const AML_HANDLER: AmlHandler = AmlHandler;
const HANDLER: Handler = Handler;

lazy_static::lazy_static! {
    static ref ACPI_TABLES: Once<AcpiTables<Handler>> = Once::new();

    static ref AML_CTX: Mutex<aml::AmlContext> = Mutex::new( aml::AmlContext::new(
        alloc::boxed::Box::new(AML_HANDLER),
        aml::DebugVerbosity::All,
    ));
}

pub fn init() {
    tables::init_rsdt();
    tables::madt::init();
    tables::fadt::init();

    unsafe {
        let rsdp_address = HANDLER
            .map_physical_region(get_rsdp_addr().expect("no rsdt").as_u64() as usize, 0x10000);

        let tables = ACPI_TABLES.call_once(|| {
            AcpiTables::from_validated_rsdp(HANDLER, rsdp_address).expect("acpi parse error")
        });

        let plinfo = tables.platform_info().expect("platform_info");
        log::debug!("plinfo.power_profile {:?}", plinfo.power_profile);
        log::debug!("plinfo.interrupt_model {:?}", plinfo.interrupt_model);

        let fadt = tables
            .get_sdt::<Fadt>(Signature::FADT)
            .expect("rsdt error")
            .expect("rsdt missing");

        log::debug!("SCI number {}", { fadt.sci_interrupt });

        // https://wiki.osdev.org/Acpi#Switching_to_ACPI_Mode
        let pm1a = fadt.pm1a_control_block().expect("pm1a cnt blk error");

        if fadt.smi_cmd_port == 0
            || (fadt.acpi_enable == 0 && fadt.acpi_disable == 0)
            || read_generic_addr(pm1a) & 1 != 0
        {
            log::debug!("ACPI mode enabled already");
        } else {
            log::debug!("Switching to ACPI mode");
            cpuio::UnsafePort::<u8>::new(fadt.smi_cmd_port as u16).write(fadt.acpi_enable);
            // TODO: maybe sleep here for a bit
            while read_generic_addr(pm1a) & 1 == 0 {
                log::debug!("Poll: switching to ACPI mode");
            }
        }

        let dsdt_aml = tables.dsdt.as_ref().expect("Missing dsdt");

        let mut aml_init_tables = vec![dsdt_aml];
        aml_init_tables.extend(tables.ssdts.iter());

        let mut ctx = AML_CTX.lock();

        for aml_table in aml_init_tables {
            let dsdt_aml_code = core::slice::from_raw_parts(
                phys_to_virt(PhysAddr::new(aml_table.address as u64)).as_ptr(),
                aml_table.length as usize,
            );

            ctx.parse_table(dsdt_aml_code).expect("exec aml dsdt");
        }

        ctx.initialize_objects().expect("init");

        // Invoke _PIC, it's optinal so ignore any errors
        let _ = ctx.invoke_method(
            &aml::AmlName::from_str("\\_PIC").unwrap(),
            Args::from_list(vec![AmlValue::Integer(1)]).expect("args"),
        );
    }
}

/// Shuts down the computer
pub fn power_off() -> ! {
    let tables = ACPI_TABLES.poll().expect("ACPI not initialized");
    log::warn!("ACPI power off");

    unsafe {
        let fadt = tables
            .get_sdt::<Fadt>(Signature::FADT)
            .expect("rsdt error")
            .expect("rsdt missing");

        // TODO: call _PTS

        asm!("cli");

        let ctx = AML_CTX.lock();
        let s5 = ctx
            .namespace
            .get_by_path(&aml::AmlName::from_str("\\_S5").unwrap())
            .expect("_S5 missing");

        let AmlValue::Package(values) = s5 else {
            panic!("Invalid _S5: pkg");
        };

        let AmlValue::Integer(slp_typa) = &values[3] else {
            panic!("Invalid _S5: int");
        };

        let a = fadt.pm1a_control_block().expect("pm1a cnt blk error");
        let sleep_enable = 1 << 13;
        let x = ((*slp_typa) << 10) | sleep_enable;

        log::debug!("{:?}", a);
        write_generic_addr(a, x);

        // TODO: See https://dox.ipxe.org/acpipwr_8c_source.html
        // let b = fadt.pm1b_control_block().expect("pm1b cnt blk error").expect("pm1b cnt blk missing");
    }

    panic!("Power off");
}

fn read_generic_addr(addr: GenericAddress) -> u64 {
    use acpi::platform::address::AddressSpace;

    match addr.address_space {
        AddressSpace::SystemMemory => todo!("generic SystemMemory"),
        AddressSpace::SystemIo => {
            assert!(addr.address <= u16::MAX as u64);
            let port = addr.address as u16;
            unsafe {
                match addr.bit_width {
                    8 => cpuio::UnsafePort::<u8>::new(port).read() as u64,
                    16 => cpuio::UnsafePort::<u16>::new(port).read() as u64,
                    32 => cpuio::UnsafePort::<u32>::new(port).read() as u64,
                    bw => panic!("Generic write: invalid bit width {}", bw),
                }
            }
        },
        AddressSpace::PciConfigSpace => todo!("generic PciConfigSpace"),
        AddressSpace::EmbeddedController => todo!("generic EmbeddedController"),
        AddressSpace::SMBus => todo!("generic SMBus"),
        AddressSpace::SystemCmos => todo!("generic SystemCmos"),
        AddressSpace::PciBarTarget => todo!("generic PciBarTarget"),
        AddressSpace::Ipmi => todo!("generic Ipmi"),
        AddressSpace::GeneralIo => todo!("generic GeneralIo"),
        AddressSpace::GenericSerialBus => todo!("generic GenericSerialBus"),
        AddressSpace::PlatformCommunicationsChannel => {
            todo!("generic PlatformCommunicationsChannel")
        },
        AddressSpace::FunctionalFixedHardware => todo!("generic FunctionalFixedHardware"),
        AddressSpace::OemDefined(v) => todo!("generic OemDefined({})", v),
    }
}

fn write_generic_addr(addr: GenericAddress, value: u64) {
    use acpi::platform::address::AddressSpace;

    match addr.address_space {
        AddressSpace::SystemMemory => todo!("generic SystemMemory"),
        AddressSpace::SystemIo => {
            assert!(addr.address <= u16::MAX as u64);
            let port = addr.address as u16;
            unsafe {
                match addr.bit_width {
                    8 => cpuio::UnsafePort::<u8>::new(port).write(value as u8),
                    16 => cpuio::UnsafePort::<u16>::new(port).write(value as u16),
                    32 => cpuio::UnsafePort::<u32>::new(port).write(value as u32),
                    bw => panic!("Generic write: invalid bit width {}", bw),
                }
            }
        },
        AddressSpace::PciConfigSpace => todo!("generic PciConfigSpace"),
        AddressSpace::EmbeddedController => todo!("generic EmbeddedController"),
        AddressSpace::SMBus => todo!("generic SMBus"),
        AddressSpace::SystemCmos => todo!("generic SystemCmos"),
        AddressSpace::PciBarTarget => todo!("generic PciBarTarget"),
        AddressSpace::Ipmi => todo!("generic Ipmi"),
        AddressSpace::GeneralIo => todo!("generic GeneralIo"),
        AddressSpace::GenericSerialBus => todo!("generic GenericSerialBus"),
        AddressSpace::PlatformCommunicationsChannel => {
            todo!("generic PlatformCommunicationsChannel")
        },
        AddressSpace::FunctionalFixedHardware => todo!("generic FunctionalFixedHardware"),
        AddressSpace::OemDefined(v) => todo!("generic OemDefined({})", v),
    }
}
