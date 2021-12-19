use core::arch::asm;

use super::device::DeviceLocation;

pub const CONFIG_ADDR: usize = 0xCF8;
pub const CONFIG_DATA: usize = 0xCFC;

/// From http://wiki.osdev.org/PCI#Configuration_Space_Access_Mechanism_.231
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

pub(crate) fn pci_read_device(loc: DeviceLocation, offset: u8) -> u32 {
    unsafe { pci_read_u32(loc.0, loc.1, loc.2, offset) }
}

pub(crate) fn pci_write_device(loc: DeviceLocation, offset: u8, value: u32) {
    unsafe { pci_write_u32(loc.0, loc.1, loc.2, offset, value) }
}
