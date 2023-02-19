#![no_std]
#![feature(allocator_api)]
#![allow(unused_imports, dead_code)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::vec::Vec;
use hashbrown::HashMap;

use libd7::net::d7net::MacAddr;
use libd7::{ipc, process::ProcessId, select, syscall};

mod dma;
mod rtl8139;

#[no_mangle]
fn main() -> ! {
    syscall::debug_print("RTL8139 driver starting");

    // Make sure this is the only NIC driver running
    libd7::service::register("exclude/nic", false);

    // Get device info
    let pci_device: Option<d7pci::Device> = ipc::request("pci/device", &"rtl8139").unwrap();
    let pci_device = pci_device.expect("PCI device resolution failed unexpectedly");

    // Initialize the driver
    let mut device = unsafe { rtl8139::RTL8139::new(pci_device) };

    // Subscribe to hardware events
    // TODO: dynamic IRQ detection (in kernel?)
    let irq = ipc::UnreliableSubscription::<()>::exact(&"irq/11").unwrap();
    // let irq = ipc::UnreliableSubscription::<()>::exact(&"irq/17").unwrap();
    // let irq = ipc::UnreliableSubscription::<u64>::exact(&format!("irq/{}", device.irq)).unwrap();

    // Subscribe to client requests
    let get_mac: ipc::Server<(), MacAddr> = ipc::Server::exact("nic/rtl8139/mac").unwrap();
    let send = ipc::UnreliableSubscription::<Vec<u8>>::exact("nic/send").unwrap();

    // Inform serviced that we are running.
    libd7::service::register("driver_rtl8139", false);

    println!("rtl Ready");

    loop {
        select! {
            one(irq) => {
                let _: () = irq.receive().unwrap();
                println!("rtl: IRQ NOTIFY");
                let received_packets = device.notify_irq();
                for packet in received_packets {
                    ipc::deliver("netd/received", &packet).unwrap();
                }
            },
            one(get_mac) => get_mac.handle(|()| Ok(device.mac_addr())).unwrap(),
            one(send) => {
                let packet: Vec<u8> = send.receive().unwrap();
                println!("rtl: SEND PKT");
                device.send(&packet);
            }
        }
    }
}
