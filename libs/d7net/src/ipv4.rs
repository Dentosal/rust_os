//! https://en.wikipedia.org/wiki/IPv4#Packet_structure

use alloc::prelude::v1::*;
use core::convert::TryFrom;

use crate::Ipv4Addr;

pub use crate::ip_protocol::IpProtocol;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub header: Header,
    pub payload: Vec<u8>,
}
impl Packet {
    pub fn from_bytes(input: &[u8]) -> Self {
        let header = Header::from_bytes(&input[..20]);
        Self {
            header,
            payload: input[20..20 + (header.payload_len as usize)].to_vec(),
        }
    }

    /// https://en.wikipedia.org/wiki/Address_Resolution_Protocol#Packet_structure
    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = Vec::new();
        todo!();
        result
    }
}

/// Does not support Options field
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub dscp_and_ecn: u8,
    pub payload_len: u16,
    pub identification: u16,
    pub flags_and_frament: u16,
    pub ttl: u8,
    pub protocol: IpProtocol,
    pub src_ip: Ipv4Addr,
    pub dst_ip: Ipv4Addr,
}
impl Header {
    pub fn from_bytes(input: &[u8]) -> Self {
        assert!(input[0] >> 4 == 4, "Version not Ipv4");
        assert!(input[0] & 0xf == 5, "IHL != 5 (Options not supported)");

        let total_len = u16::from_be_bytes([input[2], input[3]]);
        assert!(total_len >= 20, "Packet total_length too small");

        Self {
            dscp_and_ecn: input[1],
            payload_len: total_len - 20,
            identification: u16::from_be_bytes([input[4], input[5]]),
            flags_and_frament: u16::from_be_bytes([input[6], input[7]]),
            ttl: input[8],
            protocol: IpProtocol::try_from(input[9]).expect("Unknown IP protocol"),
            src_ip: Ipv4Addr::from_bytes(&input[12..16]),
            dst_ip: Ipv4Addr::from_bytes(&input[16..20]),
        }
    }

    /// https://en.wikipedia.org/wiki/Address_Resolution_Protocol#Packet_structure
    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = Vec::new();
        todo!();
        result
    }
}
