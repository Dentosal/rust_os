#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

use alloc::prelude::v1::*;

use d7net::{arp, ethernet, EtherType, Ipv4Addr, MacAddr};
use libd7::{
    d7abi::fs::protocol::network::ReceivedPacket,
    fs::{self, File},
    pinecone, syscall,
};

#[no_mangle]
fn main() -> ! {
    syscall::debug_print("Network daemon starting");

    let mut buffer = [0; 2048]; // Large enough for any network package
    let nic = File::open("/dev/nic").unwrap();

    let my_mac = {
        let mac_bytes = fs::read("/dev/nic_mac").unwrap();
        if mac_bytes.is_empty() {
            panic!("No mac address, stopping");
        }
        MacAddr::from_bytes(&mac_bytes)
    };
    let my_ip = Ipv4Addr([10, 0, 2, 15]); // Use fixed IP until DHCP is implemented

    loop {
        let count = nic.read(&mut buffer).unwrap();
        let event: ReceivedPacket = pinecone::from_bytes(&buffer[..count]).unwrap();

        let frame = ethernet::Frame::from_bytes(&event.packet);

        syscall::debug_print(&format!(
            "Received {:?} packet from {:?}",
            frame.header.ethertype, frame.header.src_mac
        ));

        match frame.header.ethertype {
            EtherType::ARP => {
                let arp_packet = arp::Packet::from_bytes(&frame.payload);
                syscall::debug_print(&format!("{:?}", arp_packet));

                if arp_packet.is_request() && arp_packet.target_ip == my_ip {
                    syscall::debug_print(&format!("Reply to"));

                    let reply = (ethernet::Frame {
                        header: ethernet::FrameHeader {
                            dst_mac: frame.header.src_mac,
                            src_mac: my_mac,
                            ethertype: EtherType::ARP,
                        },
                        payload: arp_packet.to_reply(my_mac, my_ip).to_bytes(),
                    }).to_bytes();

                    nic.write_all(&pinecone::to_vec(&reply).unwrap()).unwrap();
                }
            }
            _ => {}
        }
    }
}
