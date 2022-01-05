//! https://en.wikipedia.org/wiki/IPv4#Packet_structure

use alloc::vec::Vec;
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
        let mut result = Vec::new();
        result.push(0x45); // Version and IHL
        result.push(self.dscp_and_ecn);
        result.extend(&u16::to_be_bytes(20 + (payload_len as u16)));
        result.extend(&u16::to_be_bytes(self.identification));
        result.extend(&u16::to_be_bytes(self.flags_and_frament));
        result.push(self.ttl);
        result.push(self.protocol as u8);
        result.extend(&u16::to_be_bytes(0)); // Checksum
        result.extend(&self.src_ip.0);
        result.extend(&self.dst_ip.0);
        let checksum = crate::checksum::inet_checksum(&result);
        result[10..12].copy_from_slice(&u16::to_be_bytes(checksum));
        result
    }
}
