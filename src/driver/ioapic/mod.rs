//! https://wiki.osdev.org/IOAPIC

use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};
use x86_64::{PhysAddr, VirtAddr};

use crate::driver::acpi::ACPI_DATA;
use crate::memory;
use crate::smp::ProcessorId;

pub mod io;
pub mod lapic;

pub use self::lapic::processor_id as apic_processor_id;

static APIC_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn is_enabled() -> bool {
    APIC_ENABLED.load(Ordering::SeqCst)
}

/// TODO: move to somewhere else?
const STARTUP_CODE: &[u8] = include_bytes!("../../../build/smp_ap_startup.bin");

fn enable_local_apic() {
    // Enable APIC
    let local_apic_addr = ACPI_DATA
        .poll()
        .expect("acpi::init not called")
        .local_apic_addr;

    let addr = memory::phys_to_virt(local_apic_addr);

    // https://wiki.osdev.org/APIC#Spurious_Interrupt_Vector_Register
    let field = (addr.as_u64() + 0xf0) as *mut u32;
    unsafe {
        let value = ptr::read_volatile(field);
        ptr::write_volatile(field, value | 0xff | 0x100);
    }
}

/// Global IO APIC enable function, only ran by the BSP
pub fn init_bsp() {
    // Disable old PICs
    crate::driver::pic::disable();

    // I/O APIC initialization
    io::init();

    // Do per-processor initialization
    per_processor_init();

    // Mark APIC as enabled
    APIC_ENABLED.store(true, Ordering::SeqCst);

    // Set up startup code for other processor cores

    // TODO: addresses and sizes to constants
    let base = memory::phys_to_virt(PhysAddr::new(0x2000));

    unsafe {
        assert!(STARTUP_CODE.len() <= 0x1000); // TODO: static assert?
        ptr::copy_nonoverlapping(STARTUP_CODE.as_ptr(), base.as_mut_ptr(), STARTUP_CODE.len());
    }
}

/// LAPIC initalization, done for each processor
pub fn per_processor_init() {
    enable_local_apic();
    lapic::configure_timer(0xd8);
}

/// Wake up a CPU Core
pub fn apic_wakeup_processor(acpi_id: u8) {
    let local_apic_addr = ACPI_DATA
        .poll()
        .expect("acpi::init not called")
        .local_apic_addr;

    let addr = memory::phys_to_virt(local_apic_addr);

    // https://wiki.osdev.org/APIC#Interrupt_Command_Register
    let field_lo = (addr.as_u64() + 0x300) as *mut u32;
    let field_hi = (addr.as_u64() + 0x310) as *mut u32;

    unsafe {
        // Init IPI
        log::trace!("Sending Init IPI to core {}", acpi_id);
        ptr::write_volatile(field_hi, (acpi_id as u32) << 24);
        ptr::write_volatile(field_lo, 0x00004500);

        crate::smp::sleep::sleep_ns(10_000_000);

        // Startup IPI
        log::trace!("Sending Startup IPI to core {}", acpi_id);
        // startup addr: 0x2000
        // ^ TODO: constant for this
        ptr::write_volatile(field_hi, (acpi_id as u32) << 24);
        ptr::write_volatile(field_lo, 0x4600 | 0x0002);
    }
}

pub fn send_ipi(acpi_id: u8, int_vector: u8, synchronous: bool) {
    let local_apic_addr = ACPI_DATA
        .poll()
        .expect("acpi::init not called")
        .local_apic_addr;

    let addr = memory::phys_to_virt(local_apic_addr);

    // https://wiki.osdev.org/APIC#Interrupt_Command_Register
    let field_lo = (addr.as_u64() + 0x300) as *mut u32;
    let field_hi = (addr.as_u64() + 0x310) as *mut u32;

    unsafe {
        log::trace!("Sending IPI to core {} (vector {})", acpi_id, int_vector);
        ptr::write_volatile(field_hi, (acpi_id as u32) << 24);
        ptr::write_volatile(field_lo, int_vector as u32);

        if synchronous {
            while ptr::read_volatile(field_lo) & (1 << 12) != 0 {}
        }
    }
}

pub fn broadcast_ipi(include_self: bool, int_vector: u8) {
    let local_apic_addr = ACPI_DATA
        .poll()
        .expect("acpi::init not called")
        .local_apic_addr;

    let addr = memory::phys_to_virt(local_apic_addr);

    // https://wiki.osdev.org/APIC#Interrupt_Command_Register
    let field_lo = (addr.as_u64() + 0x300) as *mut u32;
    let field_hi = (addr.as_u64() + 0x310) as *mut u32;

    unsafe {
        log::trace!(
            "Broadcasting IPI (self: {}) (vector {})",
            include_self,
            int_vector
        );

        let mode: u32 = if include_self { 0b10 << 18 } else { 0b11 << 18 };

        ptr::write_volatile(field_hi, 0u32);
        ptr::write_volatile(field_lo, (int_vector as u32) | mode);
    }
}
