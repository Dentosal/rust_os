//! TODO: lease renew

use alloc::vec::Vec;

use libd7::ipc;
use libd7::net::d7net::*;
use libd7::random;

use super::InterfaceSettings;

/// A DHCP client
#[derive(Debug)]
pub struct Client {
    /// Transaction id, `XID` in some DHCP docs
    id: u32,
    mac_addr: MacAddr,
    state: ClientState,
}
impl Client {
    pub fn new(mac_addr: MacAddr) -> Self {
        Self {
            id: u32::from_le_bytes(random::fast_arr()),
            mac_addr,
            state: ClientState::Initial,
        }
    }

    pub fn send_discover(&mut self) {
        assert!(self.mac_addr != MacAddr::ZERO);

        let ef = ethernet::Frame {
            header: ethernet::FrameHeader {
                dst_mac: MacAddr::BROADCAST,
                src_mac: self.mac_addr,
                ethertype: EtherType::Ipv4,
            },
            payload: builder::ipv4_udp::Builder::new(
                Ipv4Addr::ZERO,
                Ipv4Addr::BROADCAST,
                68,
                67,
                dhcp::Payload::discover(self.id, self.mac_addr).to_bytes(),
            )
            .build(),
        };

        let mut packet = ef.to_bytes();
        while packet.len() < 64 {
            packet.push(0);
        }

        ipc::publish("nic/send", &packet).expect("Delivery failed");
        self.state = ClientState::Discover;
    }

    fn accept_offer(&mut self, client_ip: Ipv4Addr, server_ip: Ipv4Addr) {
        let ef = ethernet::Frame {
            header: ethernet::FrameHeader {
                dst_mac: MacAddr::BROADCAST,
                src_mac: self.mac_addr,
                ethertype: EtherType::Ipv4,
            },
            payload: builder::ipv4_udp::Builder::new(
                Ipv4Addr::ZERO,
                Ipv4Addr::BROADCAST,
                68,
                67,
                dhcp::Payload::request(self.id, self.mac_addr, client_ip, server_ip).to_bytes(),
            )
            .build(),
        };

        let mut packet = ef.to_bytes();
        while packet.len() < 64 {
            packet.push(0);
        }

        ipc::publish("nic/send", &packet).expect("Delivery failed");
        self.state = ClientState::Request;
    }

    pub fn on_packet(&mut self, packet: udp::Packet) -> Option<InterfaceSettings> {
        let payload = dhcp::Payload::from_bytes(&packet.payload);
        println!("dhcp {:?}", payload);

        if payload.op != dhcp::MsgType::REPLY {
            println!("Ignoring non-reply packet");
            return None;
        }

        let Some(op) = payload.options.iter().find_map(|opt| match opt {
            dhcp::DhcpOption::Op(op) => Some(*op),
            _ => None,
        }) else {
            println!("Ignoring packet without DHCP op");
            return None;
        };

        match self.state {
            ClientState::Initial => {
                println!("Ignoring packets in initial state");
            },
            ClientState::Discover => {
                if op == dhcp::Op::OFFER {
                    let Some(sid) = payload.options.iter().find_map(|opt| match opt {
                        dhcp::DhcpOption::ServerId(sid) => Some(*sid),
                        _ => None,
                    }) else {
                        println!("Ignoring offer without server id");
                        return None;
                    };
                    self.accept_offer(payload.your_ip, sid);
                    println!("Accepting DHCP offer for {:?}", payload.your_ip);
                } else {
                    println!("Ignoring non-offer packet after discover");
                }
            },
            ClientState::Request => {
                if op == dhcp::Op::ACK {
                    println!("DHCP state: operational");
                    self.state = ClientState::Operational;

                    return Some(InterfaceSettings {
                        ipv4: Some(payload.your_ip),
                        netmask: payload.options.iter().find_map(|opt| match opt {
                            dhcp::DhcpOption::SubnetMask(m) => Some(*m),
                            _ => None,
                        }),
                        routers: payload
                            .options
                            .iter()
                            .find_map(|opt| match opt {
                                dhcp::DhcpOption::Routers(m) => Some(m.clone()),
                                _ => None,
                            })
                            .unwrap_or_else(|| Vec::new()),
                        dns_servers: payload
                            .options
                            .iter()
                            .find_map(|opt| match opt {
                                dhcp::DhcpOption::DnsServers(m) => Some(m.clone()),
                                _ => None,
                            })
                            .unwrap_or_else(|| Vec::new()),
                    });
                } else if op == dhcp::Op::NAK {
                    todo!("DHCP: failed");
                } else {
                    println!("Ignoring non-ack packet after accept");
                }
            },
            ClientState::Operational => {
                println!("Ignoring packets in operational mode");
            },
        }

        None
    }
}

#[derive(Debug)]
enum ClientState {
    Initial,
    Discover,
    Request,
    Operational,
}
