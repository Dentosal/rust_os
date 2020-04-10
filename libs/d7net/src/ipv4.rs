//! https://en.wikipedia.org/wiki/IPv4#Packet_structure

use alloc::prelude::v1::*;

use crate::Ipv4Addr;

pub use crate::ip_protocol::IpProtocol;

/// Only supports Ethernet with MAC addresses and IPv4
/// https://en.wikipedia.org/wiki/Address_Resolution_Protocol#Packet_structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub dscp_and_ecn: u8,
    pub identification: u16,
    pub flags_and_frament: u16,
    pub ttl: u8,
    pub protocol: IpProtocol,
    pub src_ip: Ipv4Addr,
    pub dst_ip: Ipv4Addr,
}
impl Header {
    pub fn from_bytes(input: &[u8]) -> Self {
        todo!()
    }

    /// https://en.wikipedia.org/wiki/Address_Resolution_Protocol#Packet_structure
    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = Vec::new();
        todo!();
        result
    }
}
