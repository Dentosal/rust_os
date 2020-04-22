//! Networking daemon
//!
//!

#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::prelude::v1::*;
use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use libd7::{
    ipc::{self, SubscriptionId},
    net::d7net::*,
    pinecone,
    process::{Process, ProcessId},
    select, service, syscall,
    syscall::{SyscallErrorCode, SyscallResult},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Driver {
    name: String,
    path: String,
}

struct NetState {
    pub mac: MacAddr,
    pub ip: Ipv4Addr,
    pub sockets: HashMap<u64, SocketAddr>,
    pub next_socket: u64,
}
impl NetState {
    pub fn new(mac: MacAddr) -> Self {
        Self {
            mac,
            ip: Ipv4Addr([10, 0, 2, 15]), // Use fixed IP until DHCP is implemented
            sockets: HashMap::new(),
            next_socket: 1,
        }
    }

    pub fn create_socket(&mut self, addr: SocketAddr) -> u64 {
        let id = self.next_socket;
        self.next_socket += 1;
        self.sockets.insert(id, addr);
        id
    }

    pub fn on_event(&mut self, packet: &[u8]) {
        let frame = ethernet::Frame::from_bytes(&packet);

        println!(
            "Received {:?} packet from {:?}",
            frame.header.ethertype, frame.header.src_mac
        );

        match frame.header.ethertype {
            EtherType::ARP => {
                // Reply to ARP packets
                let arp_packet = arp::Packet::from_bytes(&frame.payload);
                if arp_packet.is_request() && arp_packet.target_ip == self.ip {
                    println!("ARP: Replying");

                    let reply = (ethernet::Frame {
                        header: ethernet::FrameHeader {
                            dst_mac: frame.header.src_mac,
                            src_mac: self.mac,
                            ethertype: EtherType::ARP,
                        },
                        payload: arp_packet.to_reply(self.mac, self.ip).to_bytes(),
                    })
                    .to_bytes();

                    ipc::deliver("nic/send", &reply).unwrap();
                }
            }
            EtherType::Ipv4 => {
                let ip_packet = ipv4::Packet::from_bytes(&frame.payload);
                println!("{:?}", ip_packet);

                match ip_packet.header.protocol {
                    IpProtocol::TCP => {
                        let tcp_packet = tcp::Segment::from_bytes(&ip_packet.payload);
                        println!("{:?}", tcp_packet);
                    }
                    _ => {}
                }

                panic!("IP!!!");
            }
            _ => {}
        }
    }
}

// fn handle_attachment(a: &mut attachment::BufferedAttachment, net_state: &mut NetState) {
//     let r = match a.next_request() {
//         Some(v) => v,
//         None => return,
//     }.unwrap();
//     match &r.operation {
//         attachment::RequestFileOperation::Read(count) => a.reply(r.response({
//             if let Some(path) = r.suffix.clone() {
//                 match path.as_ref() {
//                     "socket" => {
//                         // Read socket directory
//                         let mut hs = HashSet::new();
//                         for s in net_state.sockets.keys() {
//                             hs.insert(s.to_string());
//                         }
//                         attachment::ResponseFileOperation::Read(pinecone::to_vec(&hs).unwrap())
//                     }
//                     "newsocket" => attachment::ResponseFileOperation::Error(
//                         SyscallErrorCode::fs_operation_not_supported,
//                     ),
//                     other => todo!("????"),
//                 }
//             } else {
//                 // Read root branch
//                 let mut hs = HashSet::new();
//                 hs.insert("socket");
//                 hs.insert("newsocket");
//                 attachment::ResponseFileOperation::Read(pinecone::to_vec(&hs).unwrap())
//             }
//         })).unwrap(),
//         attachment::RequestFileOperation::Write(data) => {
//             if let Some(path) = r.suffix.clone() {
//                 match path.as_ref() {
//                     "socket" => a.reply(r.response(attachment::ResponseFileOperation::Error(
//                         SyscallErrorCode::fs_operation_not_supported,
//                     ))).unwrap(),
//                     "newsocket" => {
//                         // Create a new socket
//                         let addr: SocketAddr = pinecone::from_bytes(&data).unwrap();
//                         let socket_id = net_state.create_socket(addr);
//                         let response_bytes = pinecone::to_vec(&socket_id).unwrap();
//                         a.reply(r.response(attachment::ResponseFileOperation::Write(
//                             response_bytes.len() as u64
//                         ))).unwrap();
//                         a.buffer_reply(r.response(attachment::ResponseFileOperation::Read(
//                             response_bytes
//                         )));
//                     }
//                     other => todo!("????"),
//                 }
//             } else {
//                 // Read root branch
//                 let mut hs = HashSet::new();
//                 hs.insert("socket");
//                 hs.insert("newsocket");

//                 a.reply(r.response(attachment::ResponseFileOperation::Error(
//                     SyscallErrorCode::fs_operation_not_supported,
//                 ))).unwrap();
//             }
//         }
//         other => {
//             a.reply(r.response(attachment::ResponseFileOperation::Error(
//                 SyscallErrorCode::fs_operation_not_supported,
//             ))).unwrap();
//         }
//     }
// }

#[no_mangle]
fn main() -> ! {
    println!("Network daemon starting");

    // Wait until a driver is available
    service::wait_for_one("driver_rtl8139");

    let mac_addr: MacAddr = match ipc::request("nic/rtl8139/mac", &()) {
        Ok(mac) => mac,
        Err(SyscallErrorCode::ipc_delivery_no_target) => {
            panic!("No NIC drivers available");
        }
        Err(err) => panic!("NIC ping failed {:?}", err),
    };

    let mut net_state = NetState::new(mac_addr);

    // Subscribe to messages
    let a: ipc::Server<SocketAddr, u64> = ipc::Server::exact("netd/newsocket").unwrap();
    let received = ipc::ReliableSubscription::<Vec<u8>>::exact("netd/received").unwrap();

    // Announce that we are running
    libd7::service::register("netd", false);

    loop {
        println!("--> select!");
        select! {
            one(received) => {
                let packet = received.ack_receive().unwrap();
                net_state.on_event(&packet);
            },
            // one(a.inner.fd) => handle_attachment(&mut a, &mut net_state),
            error -> e => panic!("ERROR {:?}", e)
        };
    }
}
