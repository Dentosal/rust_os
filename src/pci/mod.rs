mod scan;

use collections::Vec;
use spin::Mutex;

const CONFIG_ADDR: usize = 0xCF8;
const CONFIG_DATA: usize = 0xCFC;

#[derive(Clone,Copy,PartialEq,Debug)]
pub struct DeviceLocation(pub u8, pub u8, pub u8);

#[derive(Clone,Copy,PartialEq,Debug)]
pub struct DeviceClass(pub u8, pub u8, pub u8);

#[derive(Clone,Copy,PartialEq,Debug)]
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

    pub fn read_u32(&self, offset: u8) -> u32 {
        scan::pci_read_device(self.location, offset)
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
    PCI.lock().print();
    loop {}
    rprintln!("PCI: enabled");
}
