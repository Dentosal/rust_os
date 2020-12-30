//! Handlers for different ethertype packets

use libd7::net::d7net::{ethernet, EtherType};

use super::NetState;

pub mod arp;
pub mod ipv4;

pub trait Handler {
    fn on_receive(&mut self, net: &mut NetState, frame: &ethernet::Frame);
}

pub struct Handlers {
    pub arp: arp::ArpHandler,
    pub ipv4: ipv4::Ipv4Handler,
    // TODO: ipv6: ipv6::Ipv6Handler,
}
impl Handlers {
    pub fn new() -> Self {
        Self {
            arp: self::arp::ArpHandler::new(),
            ipv4: self::ipv4::Ipv4Handler::new(),
        }
    }

    pub fn get_mut(&mut self, et: &EtherType) -> Option<&mut dyn Handler> {
        match et {
            EtherType::Arp => Some(&mut self.arp),
            EtherType::Ipv4 => Some(&mut self.ipv4),
            _ => panic!("Unsupported protocol {:?}", et)
        }
    }
}
