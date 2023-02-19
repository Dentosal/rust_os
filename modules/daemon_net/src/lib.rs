//! Networking daemon
//!
//! TODO: route broadcast packets to correct interfaces
//! TOOD: react to ARP overlap
//! TOOD: offer APIs to query interfaces

#![no_std]
#![feature(drain_filter)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use hashbrown::HashMap;

use spin::RwLock;

use serde::{Deserialize, Serialize};

use libd7::{
    ipc,
    net::{
        d7net::*,
        tcp::socket_ipc_protocol::{Bind, BindError},
        SocketId,
    },
    select, service,
};

mod arp_handler;
mod dhcp_client;
mod dns_resolver;
mod interface;
mod ports;
mod tcp_handler;

use self::dns_resolver::DnsResolver;
use self::interface::{Interface, InterfaceSettings};
use self::tcp_handler::TcpHandler;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Driver {
    name: String,
    path: String,
}

static NEXT_SOCKET_ID: AtomicU64 = AtomicU64::new(0);

fn new_socket_id() -> SocketId {
    SocketId::from_u64(NEXT_SOCKET_ID.fetch_add(1, Ordering::SeqCst))
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

    /// Default interface for outbound packets, if any available
    pub fn default_send_interface(&self) -> Option<&Interface> {
        // TODO: when virtual interfaces are added, the first one might not be valid pick anymore
        // TODO: check that this interface is ready for sending
        self.interfaces.first()
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
}

pub fn on_packet(packet: &[u8]) {
    let frame = ethernet::Frame::from_bytes(&packet);

    println!(
        "Received {:?} packet from {:?}",
        frame.header.ethertype, frame.header.src_mac
    );

    match frame.header.ethertype {
        EtherType::ARP => {
            let arp_packet = arp::Packet::from_bytes(&frame.payload);
            println!("ARP: pckt {:?}", arp_packet);
            arp_handler::handle_arp_packet(&frame, &arp_packet);
        },
        EtherType::Ipv4 => {
            let ip_packet = ipv4::Packet::from_bytes(&frame.payload);
            println!("{:?}", ip_packet.header);

            match ip_packet.header.protocol {
                IpProtocol::TCP => {
                    let tcp_segment = tcp::Segment::from_bytes(&ip_packet.payload);
                    println!("{:?}", tcp_segment);
                    let mut tcp_handler = TCP_HANDLER.write();
                    tcp_handler.handle_packet(ip_packet.header, tcp_segment);
                },
                IpProtocol::UDP => {
                    let udp_packet = udp::Packet::from_bytes(&ip_packet.payload);
                    println!("{:?}", udp_packet.header);

                    let addr_exact = SocketAddr {
                        host: IpAddr::V4(ip_packet.header.dst_ip),
                        port: udp_packet.header.dst_port,
                    };

                    let addr_any_ip = SocketAddr {
                        host: IpAddr::V4(Ipv4Addr::ZERO),
                        port: udp_packet.header.dst_port,
                    };

                    let mut net_state = NET_STATE.write();
                    if let Some(handler) = net_state
                        .udp_handlers
                        .get(&addr_exact)
                        .or(net_state.udp_handlers.get(&addr_any_ip))
                    {
                        handler(&mut net_state, frame.header, ip_packet.header, udp_packet);
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

lazy_static::lazy_static! {
    static ref NET_STATE: RwLock<NetState> = RwLock::new(NetState::new());
    static ref DNS_RESOLVER: RwLock<DnsResolver> = RwLock::new(DnsResolver::new());
    static ref TCP_HANDLER: RwLock<TcpHandler> = RwLock::new(TcpHandler::new());
}

#[no_mangle]
fn main() -> ! {
    println!("Network daemon starting");

    // TODO: Instead of enumerating NICs here, provide an
    // endpoint that NICs register when they are ready
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

    {
        let mut net_state = NET_STATE.write();
        net_state.interfaces.push(Interface::new(mac_addr));

        fn handle_udp_dhcp(
            ns: &mut NetState, e: ethernet::FrameHeader, h: ipv4::Header, p: udp::Packet,
        ) {
            println!("{:?}", ns.interfaces);
            println!("{:?}", e.dst_mac);
            let intf = ns.interface_mut(e.dst_mac).unwrap();
            intf.on_dhcp_packet(e, h, p)
        }

        net_state.udp_handlers.insert(
            SocketAddr {
                host: IpAddr::V4(Ipv4Addr::ZERO),
                port: 68,
            },
            handle_udp_dhcp,
        );

        fn handle_udp_dns(
            _: &mut NetState, _: ethernet::FrameHeader, _: ipv4::Header, p: udp::Packet,
        ) {
            let mut resolver = DNS_RESOLVER.write();
            resolver.on_packet(p)
        }

        net_state.udp_handlers.insert(
            SocketAddr {
                host: IpAddr::V4(Ipv4Addr::ZERO),
                port: ports::FIXED_DNS_CLIENT,
            },
            handle_udp_dns,
        );
    }

    // Subscribe to messages
    let get_mac: ipc::Server<(), MacAddr> = ipc::Server::exact("netd/mac").unwrap();
    let received = ipc::ReliableSubscription::<Vec<u8>>::exact("netd/received").unwrap();
    let dns_resolve =
        ipc::Server::<dns_resolver::Query, dns_resolver::Answer>::exact("netd/dns/resolve")
            .unwrap();
    // let new_socket_udp = ipc::ReliableSubscription::<()>::exact("netd/newsocket/udp").unwrap();
    let new_socket_tcp =
        ipc::Server::<Bind, Result<String, BindError>>::exact("netd/newsocket/tcp").unwrap();

    // Announce that we are running
    libd7::service::register("netd", false);

    println!("netd running {:?}", mac_addr);

    {
        let mut net_state = NET_STATE.write();

        for intf in &mut net_state.interfaces {
            println!("intf {:?}", intf);
            intf.dhcp_client.send_discover();
        }
    }

    loop {
        let mut tcp_selectors = Vec::new();
        let mut tcp_s_sockets = Vec::new();
        {
            let tcp_state = TCP_HANDLER.read();
            for (sub_id, socked_id) in tcp_state.subscriptions() {
                tcp_selectors.push(sub_id);
                tcp_s_sockets.push(socked_id);
            }
        };

        println!("--> select!");

        select! {
            any(tcp_selectors) -> index => {
                let socket_id = tcp_s_sockets[index];
                let mut tcp_handler = TCP_HANDLER.write();
                tcp_handler.user_socket_event(socket_id);
            },
            one(get_mac) => get_mac.handle(|()| Ok(mac_addr)).unwrap(),
            one(received) => {
                let packet = received.ack_receive().unwrap();
                println!("RECV {}", packet.len());
                on_packet(&packet);
            },
            one(dns_resolve) => {
                let (rctx, query) = dns_resolve.receive().unwrap();
                let mut dns_resolver = DNS_RESOLVER.write();
                dns_resolver.user_resolve(rctx, query);
            },
            one(new_socket_tcp) => {
                new_socket_tcp.handle(|bind| {
                    let mut tcp_handler = TCP_HANDLER.write();
                    // TODO: ignoring bind ip parameter for now
                    Ok(tcp_handler.new_user_socket(bind.0.port))
                }).unwrap();
            },
            // one(new_socket_udp) => {
            //     let packet = new_socket_udp.ack_receive().unwrap();
            //     todo!("User UDP sockets are not supported yet");
            // },
            error -> e => panic!("ERROR {:?}", e),
        };
    }
}
