//! https://en.wikipedia.org/wiki/IPv4#Packet_structure

use alloc::prelude::v1::*;
use core::convert::TryFrom;
use serde::{Deserialize, Serialize};

use crate::Ipv4Addr;

pub use crate::ip_protocol::IpProtocol;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

    /// Fixes header fields regarding body as necessary
    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend(&self.header.to_bytes(self.payload.len()));
        result.extend(&self.payload);
        result
    }
}

/// Does not support Options field
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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

        // TODO: verify header checksum

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

    pub fn to_bytes(self, payload_len: usize) -> Vec<u8> {
        assert!(payload_len <= (u16::MAX as usize));
        let mut result = Vec::new();
        // Version 4, IHL 20 (no options)
        result.push(0b0100_0101u8);
        result.push(self.dscp_and_ecn);
        result.extend(&(20 + (payload_len as u16)).to_be_bytes());
        result.extend(&self.identification.to_be_bytes());
        result.extend(&self.flags_and_frament.to_be_bytes());
        result.push(self.ttl);
        result.push(self.protocol as u8);
        result.extend(&0u16.to_be_bytes()); // Checksum placeholder
        result.extend(&self.src_ip.0);
        result.extend(&self.dst_ip.0);

        let checksum_bytes = crate::checksum::checksum_be(result.iter());
        result[10..12].copy_from_slice(&checksum_bytes);

        result
    }

    /// Pseudo-header for encapsulated protocol (e.g. TCP or UDP) checksum computation
    /// https://en.wikipedia.org/wiki/User_Datagram_Protocol#IPv4_pseudo_header
    pub fn pseudo_header_bytes(&self, payload_len: u16) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend(&self.src_ip.0);
        result.extend(&self.dst_ip.0);
        result.push(0u8);
        result.push(self.protocol as u8);
        result.extend(&payload_len.to_be_bytes());
        result
    }

    /// New header with sensible default settings
    pub fn new(protocol: IpProtocol, src_ip: Ipv4Addr, dst_ip: Ipv4Addr) -> Self {
        Self {
            dscp_and_ecn: 0,
            payload_len: 0,
            identification: 0,
            flags_and_frament: 0,
            ttl: 64,
            protocol,
            src_ip,
            dst_ip,
        }
    }
}
