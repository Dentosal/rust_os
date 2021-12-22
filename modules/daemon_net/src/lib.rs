//! Networking daemon
//!
//! TODO: route broadcast packets to correct interfaces
//!
//! TODO: allow multiple NICs, including virtual ones, by separating
//!     NetState to multiple interfaces and providing APIs for those

#![no_std]
#![feature(let_else)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
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

mod dhcp_client;
mod interface;

use self::interface::{Interface, InterfaceSettings};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Driver {
    name: String,
    path: String,
}

struct NetState {
    pub interfaces: Vec<Interface>,
    pub arp_table: HashMap<Ipv4Addr, MacAddr>,
    pub udp_handlers:
        HashMap<SocketAddr, fn(&mut Self, ethernet::FrameHeader, ipv4::Header, udp::Packet)>,
}
impl NetState {
    pub fn new() -> Self {
        Self {
            interfaces: Vec::new(),
            arp_table: HashMap::new(),
            udp_handlers: HashMap::new(),
        }
    }

    pub fn interface(&self, mac_addr: MacAddr) -> Option<&Interface> {
        if mac_addr == MacAddr::BROADCAST {
            return self.interfaces.get(0);
        }
        self.interfaces
            .iter()
            .find(|intf| intf.mac_addr == mac_addr)
    }

    pub fn interface_mut(&mut self, mac_addr: MacAddr) -> Option<&mut Interface> {
        if mac_addr == MacAddr::BROADCAST {
            return self.interfaces.get_mut(0);
        }
        self.interfaces
            .iter_mut()
            .find(|intf| intf.mac_addr == mac_addr)
    }

    pub fn on_event(&mut self, packet: &[u8]) {
        let frame = ethernet::Frame::from_bytes(&packet);

        println!(
            "Received {:?} packet from {:?}",
            frame.header.ethertype, frame.header.src_mac
        );

        match frame.header.ethertype {
            EtherType::ARP => {
                let arp_packet = arp::Packet::from_bytes(&frame.payload);

                // Update arp table
                if arp_packet.sender_ip != Ipv4Addr::ZERO {
                    println!(
                        "ARP: Mark owner {:?} {:?}",
                        arp_packet.sender_ip, arp_packet.sender_hw
                    );
                    self.arp_table
                        .insert(arp_packet.sender_ip, arp_packet.sender_hw);
                }

                // Reply to ARP packets if the corresponding interface has an ip
                if let Some(intf) = self.interface(frame.header.dst_mac) {
                    if !intf.arp_probe_ok {
                        return;
                    }
                    if let Some(ip) = intf.settings.ipv4 {
                        if arp_packet.is_request() && arp_packet.target_ip == ip {
                            println!("ARP: Replying");

                            let reply = (ethernet::Frame {
                                header: ethernet::FrameHeader {
                                    dst_mac: frame.header.src_mac,
                                    src_mac: intf.mac_addr,
                                    ethertype: EtherType::ARP,
                                },
                                payload: arp_packet.to_reply(intf.mac_addr, ip).to_bytes(),
                            })
                            .to_bytes();

                            ipc::deliver("nic/send", &reply).unwrap();
                        }
                    }
                }
            },
            EtherType::Ipv4 => {
                let ip_packet = ipv4::Packet::from_bytes(&frame.payload);
                println!("{:?}", ip_packet);

                match ip_packet.header.protocol {
                    IpProtocol::TCP => {
                        let tcp_packet = tcp::Segment::from_bytes(&ip_packet.payload);
                        println!("{:?}", tcp_packet);
                    },
                    IpProtocol::UDP => {
                        let udp_packet = udp::Packet::from_bytes(&ip_packet.payload);
                        println!("{:?}", udp_packet);

                        let addr_exact = SocketAddr {
                            host: IpAddr::V4(ip_packet.header.dst_ip),
                            port: udp_packet.header.dst_port,
                        };

                        let addr_any_ip = SocketAddr {
                            host: IpAddr::V4(Ipv4Addr::ZERO),
                            port: udp_packet.header.dst_port,
                        };

                        if let Some(handler) = self
                            .udp_handlers
                            .get(&addr_exact)
                            .or(self.udp_handlers.get(&addr_any_ip))
                        {
                            handler(self, frame.header, ip_packet.header, udp_packet);
                            return;
                        } else {
                            println!("No UDP handlers assigned for {:?}", addr_exact);
                        }
                    },
                    _ => {},
                }
            },
            _ => {},
        }
    }
}

#[no_mangle]
fn main() -> ! {
    println!("Network daemon starting");

    let nics = ["ne2k", "rtl8139"];

    // Wait until a driver is available
    let drivers: &[String] = &nics.map(|nic| format!("driver_{}", nic));
    service::wait_for_any(drivers);

    let mut mac_addr: Option<MacAddr> = None;
    for nic in nics {
        if let Ok(addr) = ipc::request(&format!("nic/{}/mac", nic), &()) {
            mac_addr = Some(addr);
            break;
        };
    }
    let Some(mac_addr) = mac_addr else {
        panic!("No MAC address received");
    };

    let mut net_state = NetState::new();
    net_state.interfaces.push(Interface::new(mac_addr));

    fn handle_udp_dhcp(
        ns: &mut NetState, e: ethernet::FrameHeader, h: ipv4::Header, p: udp::Packet,
    ) {
        println!("{:?}", ns.interfaces);
        println!("{:?}", e.dst_mac);
        let mut intf = ns.interface_mut(e.dst_mac).unwrap();
        intf.on_dhcp_packet(e, h, p)
    }

    net_state.udp_handlers.insert(
        SocketAddr {
            host: IpAddr::V4(Ipv4Addr::ZERO),
            port: 68,
        },
        handle_udp_dhcp,
    );

    // Subscribe to messages
    let get_mac: ipc::Server<(), MacAddr> = ipc::Server::exact("netd/mac").unwrap();
    let received = ipc::ReliableSubscription::<Vec<u8>>::exact("netd/received").unwrap();

    // Announce that we are running
    libd7::service::register("netd", false);

    println!("netd running {:?}", mac_addr);

    for intf in &mut net_state.interfaces {
        intf.dhcp_client.send_discover();
    }

    loop {
        println!("--> select!");
        select! {
            one(get_mac) => get_mac.handle(|()| Ok(mac_addr)).unwrap(),
            one(received) => {
                let packet = received.ack_receive().unwrap();
                println!("RECV {}", packet.len());
                net_state.on_event(&packet);
            },
            error -> e => panic!("ERROR {:?}", e)
        };
    }
}
