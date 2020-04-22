use super::device::{Device, DeviceClass, DeviceLocation};
use super::util::pci_read_device;

use alloc::vec::Vec;

// http://wiki.osdev.org/PCI#PCI_Device_Structure

fn get_vendor_id(loc: DeviceLocation) -> u16 {
    (pci_read_device(loc, 0x0) & 0x0000FFFF) as u16
}

fn get_device_id(loc: DeviceLocation) -> u16 {
    ((pci_read_device(loc, 0x0) & 0xFFFF0000) >> 16) as u16
}

fn get_devclass(loc: DeviceLocation) -> DeviceClass {
    let d = pci_read_device(loc, 0x8);
    DeviceClass(
        ((d & 0xFF000000) >> 24) as u8,
        ((d & 0x00FF0000) >> 16) as u8,
        ((d & 0x0000FF00) >> 8) as u8,
    )
}

fn get_header_type(loc: DeviceLocation) -> u8 {
    ((pci_read_device(loc, 0xC) & 0x00FF0000) >> 16) as u8
}

fn get_secondary_bus(loc: DeviceLocation) -> u8 {
    ((pci_read_device(loc, 0x18) & 0x0000FF00) >> 8) as u8
}

fn check_device(bus: u8, dev: u8) -> Vec<Device> {
    let mut result = Vec::new();
    let vendor_id = get_vendor_id(DeviceLocation(bus, dev, 0));
    if vendor_id != 0xFFFF {
        // Device exists
        result.extend(check_function(DeviceLocation(bus, dev, 0)));
        let header_type = get_header_type(DeviceLocation(bus, dev, 0));
        if (header_type & 0x80) != 0 {
            // This is a multi-function device, so check remaining functions

            for f in 1..=8 {
                if get_vendor_id(DeviceLocation(bus, dev, f)) != 0xFFFF {
                    result.extend(check_function(DeviceLocation(bus, dev, f)));
                }
            }
        }
    }
    result
}

fn check_function(loc: DeviceLocation) -> Vec<Device> {
    let mut result = Vec::new();
    let dc = get_devclass(loc);
    result.push(Device::new(get_device_id(loc), get_vendor_id(loc), loc, dc));

    if dc.0 == 0x06 && dc.1 == 0x04 {
        let secondary_bus: u8 = get_secondary_bus(loc);
        result.extend(check_bus(secondary_bus));
    }
    result
}

fn check_bus(bus: u8) -> Vec<Device> {
    (0..=32)
        .map(|dev| check_device(bus, dev))
        .flatten()
        .collect()
}

pub unsafe fn list_devices() -> Vec<Device> {
    let header_type = get_header_type(DeviceLocation(0, 0, 0));
    if (header_type & 0x80) == 0 {
        // A single PCI host controller
        check_bus(0)
    } else {
        let mut result = Vec::new();
        // Multiple PCI host controllers
        for func in 0..=8 {
            if get_vendor_id(DeviceLocation(0, 0, func)) != 0xFFFF {
                break;
            }
            // TODO: explain, and just check_bus(bus)
            let bus: u8 = func;
            result.extend(check_bus(bus));
        }

        result
    }
}
