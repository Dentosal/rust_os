use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use libd7::ipc;
use libd7::net::d7net::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct InterfaceSettings {
    pub ipv4: Option<Ipv4Addr>,
    pub netmask: Option<Ipv4Addr>,
    pub routers: Vec<Ipv4Addr>,
    pub dns_servers: Vec<Ipv4Addr>,
}
impl InterfaceSettings {
    pub fn new() -> Self {
        Self {
            ipv4: None,
            netmask: None,
            routers: Vec::new(),
            dns_servers: Vec::new(),
        }
    }
}

/// TODO: support virtual interfaces
#[derive(Debug)]
pub struct Interface {
    pub mac_addr: MacAddr,
    pub settings: InterfaceSettings,
    /// TODO: proper arp probing with a timeout
    pub dhcp_client: crate::dhcp_client::Client,
    pub arp_probe_ok: bool,
}
impl Interface {
    pub fn new(mac_addr: MacAddr) -> Self {
        Self {
            mac_addr,
            settings: InterfaceSettings::new(),
            dhcp_client: crate::dhcp_client::Client::new(mac_addr),
            arp_probe_ok: false,
        }
    }

    /// Sends out arp probe for the current IP address
    pub fn arp_probe(&mut self) {
        let Some(ip) = self.settings.ipv4 else {
            panic!("Cannot send ARP probe: no ip configured")
        };

        let ef = ethernet::Frame {
            header: ethernet::FrameHeader {
                dst_mac: MacAddr::BROADCAST,
                src_mac: self.mac_addr,
                ethertype: EtherType::ARP,
            },
            payload: (arp::Packet {
                ptype: EtherType::Ipv4,
                operation: arp::Operation::Request,
                src_hw: self.mac_addr,
                src_ip: Ipv4Addr::ZERO,
                dst_hw: MacAddr::ZERO,
                dst_ip: ip,
            })
            .to_bytes(),
        };

        let mut packet = ef.to_bytes();
        while packet.len() < 64 {
            packet.push(0);
        }

        ipc::publish("nic/send", &packet).expect("Delivery failed");

        self.arp_probe_ok = true; // TODO: timeout
        println!("Interface {:?} online", self.mac_addr);
        self.arp_router();
    }

    /// Sends out arp probe for the current router IP
    pub fn arp_router(&mut self) {
        let Some(src_ip) = self.settings.ipv4 else {return;};

        for router_ip in &self.settings.routers {
            log::debug!("ARP-lookup for router {}", router_ip);

            let ef = ethernet::Frame {
                header: ethernet::FrameHeader {
                    dst_mac: MacAddr::BROADCAST,
                    src_mac: self.mac_addr,
                    ethertype: EtherType::ARP,
                },
                payload: (arp::Packet {
                    ptype: EtherType::Ipv4,
                    operation: arp::Operation::Request,
                    src_hw: self.mac_addr,
                    src_ip,
                    dst_hw: MacAddr::ZERO,
                    dst_ip: *router_ip,
                })
                .to_bytes(),
            };

            let mut packet = ef.to_bytes();
            while packet.len() < 64 {
                packet.push(0);
            }

            ipc::publish("nic/send", &packet).expect("Delivery failed");
        }
    }

    pub fn on_dhcp_packet(&mut self, _: ethernet::FrameHeader, _: ipv4::Header, p: udp::Packet) {
        if let Some(new_settings) = self.dhcp_client.on_packet(p) {
            self.settings = new_settings;
            self.arp_probe();
        }
    }
}
