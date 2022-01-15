//! Starts drivers for found PCI devices,
//! and then reponds to device queries

#![no_std]
#![feature(allocator_api)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use hashbrown::HashMap;
use serde::Deserialize;

use libd7::{ipc, process::Process, syscall};

#[derive(Debug, Deserialize)]
struct ConfigDevice {
    shortname: Option<String>,
    name: String,
    driver: Option<ConfigDeviceDriver>,
}

#[derive(Debug, Deserialize)]
struct ConfigDeviceDriver {
    from_initrd: bool,
    executable: String,
}

#[no_mangle]
fn main() -> ! {
    syscall::debug_print("PCI driver starting");

    // (DriverName|"vendor:id") -> d7pci::Device
    let server: ipc::Server<String, Option<d7pci::Device>> =
        ipc::Server::exact("pci/device").unwrap();

    libd7::service::register("driver_pci", false);

    let s: Vec<u8> = ipc::request("initrd/read", "pci_devices.json".to_owned()).unwrap();
    let config_devices: HashMap<String, ConfigDevice> = serde_json::from_slice(&s).unwrap();

    let devices = unsafe { d7pci::list_devices() };

    for device in &devices {
        let vendor_and_id = format!("{:x}:{:x}", device.vendor, device.id);

        if let Some(device_config) = config_devices.get(&vendor_and_id) {
            println!(
                "PCI device: {}/{:?} {} ({})",
                vendor_and_id,
                device.location,
                device_config.name,
                device_config
                    .driver
                    .as_ref()
                    .map(|d| d.executable.as_ref())
                    .unwrap_or("no driver")
            );
            if let Some(driver) = &device_config.driver {
                assert!(
                    driver.from_initrd,
                    "Non-initrd executables are not supported yet"
                );
                Process::spawn(&driver.executable, &[]).unwrap();
            }
        } else {
            println!("Ignoring unknown PCI device {}", vendor_and_id);
        }
    }

    loop {
        server
            .handle(|name| {
                for device in &devices {
                    let vendor_and_id = format!("{:x}:{:x}", device.vendor, device.id);
                    if name == vendor_and_id {
                        return Ok(Some(device.clone()));
                    }

                    if let Some(device_config) = config_devices.get(&vendor_and_id) {
                        if name == device_config.name
                            || Some(&name) == device_config.shortname.as_ref()
                        {
                            return Ok(Some(device.clone()));
                        }
                    }
                }

                Ok(None)
            })
            .unwrap();
    }
}
