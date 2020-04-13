#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::prelude::v1::*;

use d7net::{arp, ethernet, ipv4, tcp, EtherType, IpProtocol, Ipv4Addr, MacAddr};
use libd7::{
    attachment,
    d7abi::fs::protocol::network::{OutboundPacket, ReceivedPacket},
    fs::{self, File},
    pinecone, select, syscall,
};

struct NetState {
    pub nic: File,
    pub mac: MacAddr,
    pub ip: Ipv4Addr,
    pub next_socket: u64,
}
impl NetState {
    pub fn new() -> Self {
        let mac = {
            let mac_bytes = fs::read("/dev/nic_mac").unwrap();
            if mac_bytes.is_empty() {
                panic!("no mac address, stopping");
            }
            MacAddr::from_bytes(&mac_bytes)
        };

        Self {
            nic: File::open("/dev/nic").unwrap(),
            mac,
            ip: Ipv4Addr([10, 0, 2, 15]), // Use fixed IP until DHCP is implemented
            next_socket: 1,
        }
    }

    pub fn get_new_socket_name(&mut self) -> String {
        let result = format!("socket{}", self.next_socket);
        self.next_socket += 1;
        result
    }

    pub fn on_event(&mut self) {
        let mut buffer = [0; 2048]; // Large enough for any network package
        let count = self.nic.read(&mut buffer).unwrap();
        let event: ReceivedPacket = pinecone::from_bytes(&buffer[..count]).unwrap();

        let frame = ethernet::Frame::from_bytes(&event.packet);

        syscall::debug_print(&format!(
            "Received {:?} packet from {:?}",
            frame.header.ethertype, frame.header.src_mac
        ));

        match frame.header.ethertype {
            EtherType::ARP => {
                // Reply to ARP packets
                let arp_packet = arp::Packet::from_bytes(&frame.payload);
                if arp_packet.is_request() && arp_packet.target_ip == self.ip {
                    syscall::debug_print("ARP: Replying");

                    let reply = (ethernet::Frame {
                        header: ethernet::FrameHeader {
                            dst_mac: frame.header.src_mac,
                            src_mac: self.mac,
                            ethertype: EtherType::ARP,
                        },
                        payload: arp_packet.to_reply(self.mac, self.ip).to_bytes(),
                    })
                    .to_bytes();

                    self.nic
                        .write_all(&pinecone::to_vec(&OutboundPacket { packet: reply }).unwrap())
                        .unwrap();
                }
            }
            EtherType::Ipv4 => {
                let ip_packet = ipv4::Packet::from_bytes(&frame.payload);
                syscall::debug_print(&format!("{:?}", ip_packet));

                match ip_packet.header.protocol {
                    IpProtocol::TCP => {
                        let tcp_packet = tcp::Segment::from_bytes(&ip_packet.payload);
                        syscall::debug_print(&format!("{:?}", tcp_packet));
                    }
                    _ => {}
                }

                panic!("IP!!!");
            }
            _ => {}
        }
    }
}

#[no_mangle]
fn main() -> ! {
    syscall::debug_print("Network daemon starting");

    let mut net_state = NetState::new();

    let mut a_root = attachment::StaticBranch::new("/srv/net").unwrap();
    let mut a_sockets = a_root.add_branch("socket").unwrap();
    let mut a_newsocket = a_root.add_branch("newsocket").unwrap();

    // Announce that we are running
    File::open("/srv/service").unwrap().write_all(&[1]).unwrap();

    loop {
        println!("--> select!");
        select! {
            one(net_state.nic.fd) => net_state.on_event(),
            one(a_root.inner.fd) => a_root.process_one().unwrap(),
            one(a_sockets.fd) => {
                println!("--> sockets");
                let r = a_sockets.next_request();
                panic!("sockets: {:?}", r);
            },
            one(a_newsocket.fd) => {
                println!("--> newsocket");
                let r = a_newsocket.next_request();
                panic!("newsocket: {:?}", r);
            },
            error -> e => panic!("ERROR {:?}", e)
        };
    }
}
