#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![feature(no_more_cas)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::prelude::v1::*;
use hashbrown::HashMap;

use libd7::net::d7net::MacAddr;
use libd7::{ipc, process::ProcessId, select, syscall};

mod dma;
mod irq_handler;
mod rtl8139;

#[no_mangle]
fn main() -> ! {
    syscall::debug_print("RTL8139 driver starting");

    // Make sure this is the only NIC driver running
    libd7::service::register("exclude/nic", false);

    // Get device info
    let pci_device: Option<d7pci::Device> = ipc::request("pci/device", &"rtl8139").unwrap();
    let pci_device = pci_device.expect("PCI device resoltion failed unexpectedly");

    // Initialize the driver
    let mut device = unsafe { rtl8139::RTL8139::new(pci_device) };

    // Subscribe to hardware events
    let irq = ipc::UnreliableSubscription::<u64>::exact(&format!("irq/{}", device.irq)).unwrap();

    // Subscribe to client requests
    let get_mac: ipc::Server<(), MacAddr> = ipc::Server::exact("nic/rtl8139/mac").unwrap();
    let send = ipc::ReliableSubscription::<Vec<u8>>::exact("nic/send").unwrap();

    // Inform serviced that we are running.
    libd7::service::register("driver_rtl8139", false);

    loop {
        select! {
            one(irq) => {
                let status: u64 = irq.receive().unwrap();
                println!("IRQ NOTIFY");
                let received_packets = device.notify_irq(status as u16);
                for packet in received_packets {
                    ipc::deliver("netd/received", &packet).unwrap();
                }
            },
            one(get_mac) => get_mac.handle(|()| Ok(device.mac_addr())).unwrap(),
            one(send) => {
                let (ack_ctx, packet): (_, Vec<u8>) = send.receive().unwrap();
                device.send(&packet);
                ack_ctx.ack().unwrap();
            }
        }
    }
}
