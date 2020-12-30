//! https://en.wikipedia.org/wiki/IPv6_packet#Fixed_header

use alloc::prelude::v1::*;
use serde::{Deserialize, Serialize};

use crate::Ipv6Addr;

pub use crate::ip_protocol::IpProtocol;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Packet {
    pub header: Header,
    pub payload: Vec<u8>,
}
impl Packet {
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

/// Does not support Options field
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct Header {
    pub traffic_class: u8,
    pub flow_label: u32,
    pub payload_len: u16,
    pub protocol: IpProtocol,
    pub hop_limit: u8,
    pub src_ip: Ipv6Addr,
    pub dst_ip: Ipv6Addr,
}
impl Header {
    pub fn from_bytes(input: &[u8]) -> Self {
        assert!(input[0] >> 4 == 4, "Version not Ipv4");
        todo!()
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = Vec::new();
        todo!();
        result
    }
}
