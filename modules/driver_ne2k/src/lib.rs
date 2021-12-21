#![no_std]
#![feature(allocator_api)]
#![feature(no_more_cas)]
#![feature(asm)]
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
mod ne2k;
mod rtl8139;

#[no_mangle]
fn main() -> ! {
    syscall::debug_print("RTL8139 driver starting");

    // Make sure this is the only NIC driver running
    libd7::service::register("exclude/nic", false);

    // Get device info
    let pci_device: Option<d7pci::Device> = ipc::request("pci/device", &"rtl8139").unwrap();

    // XXX: bochs ne2k workaround
    let pci_device = if let Some(d) = pci_device {
        Some(d)
    } else {
        ipc::request("pci/device", &"rtl8029").unwrap()
    };

    let pci_device = pci_device.expect("PCI device resolution failed unexpectedly");

    // Initialize the driver
    // let mut device = unsafe { rtl8139::RTL8139::new(pci_device) };
    let mut device = unsafe { ne2k::Ne2k::new(pci_device) };

    // Subscribe to hardware events
    // TODO: dynamic IRQ detection (in kernel?)
    let irq = ipc::UnreliableSubscription::<()>::exact(&"irq/17").unwrap();
    // let irq = ipc::UnreliableSubscription::<u64>::exact(&format!("irq/{}", device.irq)).unwrap();

    // Subscribe to client requests
    let get_mac: ipc::Server<(), MacAddr> = ipc::Server::exact("nic/rtl8139/mac").unwrap();
    let send = ipc::ReliableSubscription::<Vec<u8>>::exact("nic/send").unwrap();

    // Inform serviced that we are running.
    libd7::service::register("driver_rtl8139", false);

    println!("rtl Ready");

    loop {
        select! {
            one(irq) => {
                let _: () = irq.receive().unwrap();
                println!("IRQ NOTIFY");
                let received_packets = device.notify_irq();
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
