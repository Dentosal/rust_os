use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use super::util;

/// Bus, device, function
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[repr(C)]
pub struct DeviceLocation(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[repr(C)]
pub struct DeviceClass(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[repr(C)]
pub struct Device {
    pub id: u16,
    pub vendor: u16,
    pub location: DeviceLocation,
    pub class: DeviceClass,
}
impl Device {
    /// Only one instance of a device must be in use at any given moment
    pub fn new(id: u16, vendor: u16, location: DeviceLocation, class: DeviceClass) -> Device {
        Device {
            id,
            vendor,
            location,
            class,
        }
    }

    pub unsafe fn read(&self, offset: u8) -> u32 {
        util::pci_read_device(self.location, offset)
    }

    pub unsafe fn write(&self, offset: u8, value: u32) {
        util::pci_write_device(self.location, offset, value)
    }

    pub unsafe fn read_u16(&self, offset: u8) -> u16 {
        assert!(offset & 1 == 0, "Must align at u16 boundary");
        let data = util::pci_read_device(self.location, offset & !0b11);
        ((data >> (offset as u32 & 0b1)) & 0xffff) as u16
    }

    pub unsafe fn read_u8(&self, offset: u8) -> u8 {
        let data = util::pci_read_device(self.location, offset & !0b11);
        ((data >> (8 * (offset as u32 & 0b11))) & 0xff) as u8
    }

    pub fn status(&self) -> u16 {
        (unsafe { self.read(0x04) } >> 16) as u16
    }

    /// Read the linked list of capabilities, filter by type
    /// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-740004
    pub fn read_capabilities<T>(&self, cap_type: u8, f: &dyn Fn(&Self, u8) -> T) -> Vec<T> {
        assert!((self.status() & (1 << 4)) != 0, "Capabilities not availble");
        let mut cap_addr = (unsafe { self.read(0x34) } & 0b1111_1100) as u8;

        let mut result = Vec::new();
        while cap_addr != 0 {
            unsafe {
                let item_type = self.read_u8(cap_addr);
                let item_size = self.read_u8(cap_addr + 2);

                if item_type == cap_type {
                    result.push(f(&self, cap_addr));
                }

                cap_addr = self.read_u8(cap_addr + 1);
            }
        }
        result
    }

    pub fn subsystem_id(&self) -> u16 {
        (unsafe { self.read(0x2c) } >> 16) as u16
    }

    pub fn get_bar(&self, i: u8) -> u32 {
        assert!(i < 6);
        unsafe { self.read(0x10 + 4 * i) }
    }

    // https://wiki.osdev.org/PCI#PCI_Device_Structure
    // Under "Interrupt Line"
    pub fn get_interrupt_line(&self) -> Option<u8> {
        let line = unsafe { self.read_u8(0x3c) };
        if line == 0xff {
            // No connection
            None
        } else {
            Some(line)
        }
    }

    // https://wiki.osdev.org/PCI#PCI_Device_Structure
    // Under "Interrupt Pin"
    pub fn get_interrupt_pin(&self) -> Option<u8> {
        let pin = unsafe { self.read_u8(0x3d) };
        if pin == 0 {
            // No pin connection
            None
        } else if pin > 4 {
            panic!("Interrupt pin value invalid");
        } else {
            Some(pin)
        }
    }

    // http://wiki.osdev.org/RTL8139#PCI_Bus_Mastering
    pub unsafe fn enable_bus_mastering(&self) {
        self.write(0x04, self.read(0x04) | (1 << 2));
    }
}
