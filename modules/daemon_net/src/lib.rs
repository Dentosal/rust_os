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

use libd7::{
    attachment,
    d7abi::{
        fs::protocol::network::{OutboundPacket, ReceivedPacket},
        SyscallErrorCode,
    },
    fs::{self, File},
    net::d7net::*,
    pinecone, select, syscall,
};

struct NetState {
    pub nic: File,
    pub mac: MacAddr,
    pub ip: Ipv4Addr,
    pub sockets: HashMap<u64, SocketAddr>,
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

fn handle_attachment(a: &mut attachment::BufferedAttachment, net_state: &mut NetState) {
    let r = match a.next_request() {
        Some(v) => v,
        None => return,
    }.unwrap();
    match &r.operation {
        attachment::RequestFileOperation::Read(count) => a.reply(r.response({
            if let Some(path) = r.suffix.clone() {
                match path.as_ref() {
                    "socket" => {
                        // Read socket directory
                        let mut hs = HashSet::new();
                        for s in net_state.sockets.keys() {
                            hs.insert(s.to_string());
                        }
                        attachment::ResponseFileOperation::Read(pinecone::to_vec(&hs).unwrap())
                    }
                    "newsocket" => attachment::ResponseFileOperation::Error(
                        SyscallErrorCode::fs_operation_not_supported,
                    ),
                    other => todo!("????"),
                }
            } else {
                // Read root branch
                let mut hs = HashSet::new();
                hs.insert("socket");
                hs.insert("newsocket");
                attachment::ResponseFileOperation::Read(pinecone::to_vec(&hs).unwrap())
            }
        })).unwrap(),
        attachment::RequestFileOperation::Write(data) => {
            if let Some(path) = r.suffix.clone() {
                match path.as_ref() {
                    "socket" => a.reply(r.response(attachment::ResponseFileOperation::Error(
                        SyscallErrorCode::fs_operation_not_supported,
                    ))).unwrap(),
                    "newsocket" => {
                        // Create a new socket
                        let addr: SocketAddr = pinecone::from_bytes(&data).unwrap();
                        let socket_id = net_state.create_socket(addr);
                        let response_bytes = pinecone::to_vec(&socket_id).unwrap();
                        a.reply(r.response(attachment::ResponseFileOperation::Write(
                            response_bytes.len() as u64
                        ))).unwrap();
                        a.buffer_reply(r.response(attachment::ResponseFileOperation::Read(
                            response_bytes
                        )));
                    }
                    other => todo!("????"),
                }
            } else {
                // Read root branch
                let mut hs = HashSet::new();
                hs.insert("socket");
                hs.insert("newsocket");

                a.reply(r.response(attachment::ResponseFileOperation::Error(
                    SyscallErrorCode::fs_operation_not_supported,
                ))).unwrap();
            }
        }
        other => {
            a.reply(r.response(attachment::ResponseFileOperation::Error(
                SyscallErrorCode::fs_operation_not_supported,
            ))).unwrap();
        }
    }
}

#[no_mangle]
fn main() -> ! {
    syscall::debug_print("Network daemon starting");

    let mut net_state = NetState::new();

    let mut a = attachment::Attachment::new_branch("/srv/net")
        .unwrap()
        .buffered();

    // Announce that we are running
    File::open("/srv/service").unwrap().write_all(&[1]).unwrap();

    loop {
        println!("--> select!");
        select! {
            one(net_state.nic.fd) => net_state.on_event(),
            one(a.inner.fd) => handle_attachment(&mut a, &mut net_state),
            error -> e => panic!("ERROR {:?}", e)
        };
    }
}
