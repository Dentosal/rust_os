mod scan;
mod util;

use alloc::vec::Vec;
use spin::Mutex;

const CONFIG_ADDR: usize = 0xCF8;
const CONFIG_DATA: usize = 0xCFC;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceLocation(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceClass(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Device {
    pub id: u16,
    pub vendor: u16,
    pub location: DeviceLocation,
    pub class: DeviceClass
}
impl Device {
    fn new(id: u16, vendor: u16, location: DeviceLocation, class: DeviceClass) -> Device {
        Device {
            id: id,
            vendor: vendor,
            location: location,
            class: class
        }
    }

    pub unsafe fn read(&self, offset: u8) -> u32 {
        util::pci_read_device(self.location, offset)
    }

    pub unsafe fn write(&self, offset: u8, value: u32) {
        util::pci_write_device(self.location, offset, value)
    }

    pub fn subsystem_id(&self) -> u16 {
        (unsafe { self.read(0x2c) } >> 16) as u16
    }

    pub fn get_bars(&self) -> [u32; 6] {
        unsafe {
            [
                self.read(0x10),
                self.read(0x14),
                self.read(0x18),
                self.read(0x1C),
                self.read(0x20),
                self.read(0x24)
            ]
        }
    }

    // http://wiki.osdev.org/RTL8139#PCI_Bus_Mastering
    pub unsafe fn enable_bus_mastering(&self) {
        self.write(0x04, self.read(0x04) | (1 << 2));
    }
}

pub struct PCIController {
    devices: Option<Vec<Device>>
}
impl PCIController {
    const fn new() -> PCIController {
        PCIController {
            devices: None
        }
    }

    fn init(&mut self) {
        self.devices = Some(scan::check_all_busses());
    }

    fn print(&self) {
        assert!(self.devices.is_some(), "PCI is not initialized");
        for dev in self.devices.clone().unwrap() {
            rprintln!("/ {:x}:{:x}", dev.vendor, dev.id);
            rprintln!("\\ {:x} {:x} {:x}", dev.class.0, dev.class.1, dev.class.2);
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
    // PCI.lock().print();
    rprintln!("PCI: enabled");
}
