mod scan;
mod util;

use alloc::vec::Vec;
use spin::Mutex;

const CONFIG_ADDR: usize = 0xCF8;
const CONFIG_DATA: usize = 0xCFC;

struct CapabilityHeader {}

/// Bus, device, function
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceLocation(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceClass(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Device {
    pub id: u16,
    pub vendor: u16,
    pub location: DeviceLocation,
    pub class: DeviceClass,
}
impl Device {
    fn new(id: u16, vendor: u16, location: DeviceLocation, class: DeviceClass) -> Device {
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

    // http://wiki.osdev.org/RTL8139#PCI_Bus_Mastering
    pub unsafe fn enable_bus_mastering(&self) {
        self.write(0x04, self.read(0x04) | (1 << 2));
    }
}

pub struct PCIController {
    devices: Option<Vec<Device>>,
}
impl PCIController {
    const fn new() -> PCIController {
        PCIController { devices: None }
    }

    fn init(&mut self) {
        self.devices = Some(scan::check_all_busses());
    }

    fn print(&self) {
        assert!(self.devices.is_some(), "PCI is not initialized");
        for dev in self.devices.clone().unwrap() {
            log::debug!("/ {:x}:{:x}", dev.vendor, dev.id);
            log::debug!("\\ {:x} {:x} {:x}", dev.class.0, dev.class.1, dev.class.2);
        }
    }

    pub fn find<P>(&self, pred: P) -> Option<Device>
    where P: Fn(Device) -> bool {
        for dev in self.devices.clone().unwrap() {
            if pred(dev) {
                return Some(dev);
            }
        }
        None
    }
}

pub static PCI: Mutex<PCIController> = Mutex::new(PCIController::new());

pub fn init() {
    PCI.lock().init();
    PCI.lock().print();
    log::info!("PCI: enabled");
}
