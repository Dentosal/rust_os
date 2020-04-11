//! https://en.wikipedia.org/wiki/Address_Resolution_Protocol

use alloc::prelude::v1::*;
use serde::{Deserialize, Serialize};

use crate::{EtherType, Ipv4Addr, MacAddr};

/// Only supports Ethernet with MAC addresses and IPv4
/// https://en.wikipedia.org/wiki/Address_Resolution_Protocol#Packet_structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct Packet {
    pub ptype: EtherType,
    pub operation: Operation,
    pub sender_hw: MacAddr,
    pub sender_ip: Ipv4Addr,
    pub target_hw: MacAddr,
    pub target_ip: Ipv4Addr,
}
impl Packet {
    pub fn from_bytes(input: &[u8]) -> Self {
        assert!(&input[..2] == [0, 1]); // Ethernet only

        let hlen = input[4] as usize;
        let plen = input[5] as usize;

        assert!(hlen == 6); // Mac addresses only
        assert!(plen == 4); // Ipv4

        Self {
            ptype: EtherType::from_bytes(&input[2..4]),
            operation: Operation::from_bytes(&input[6..8]),
            sender_hw: MacAddr::from_bytes(&input[8..8 + hlen]),
            sender_ip: Ipv4Addr::from_bytes(&input[8 + hlen..8 + hlen + plen]),
            target_hw: MacAddr::from_bytes(&input[8 + hlen + plen..8 + hlen * 2 + plen]),
            target_ip: Ipv4Addr::from_bytes(&input[8 + hlen * 2 + plen..8 + hlen * 2 + plen * 2]),
        }
    }

    /// https://en.wikipedia.org/wiki/Address_Resolution_Protocol#Packet_structure
    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = Vec::new();
        // HTYPE: Ethernet
        result.push(0);
        result.push(1);
        // PTYPE: Ipv4
        result.push(8);
        result.push(0);
        // HLEN: 6 (MacAddr)
        result.push(6);
        // PLEN: 4 (Ipv4)
        result.push(4);
        // Operation
        result.push(0);
        result.push(match self.operation {
            Operation::Request => 1,
            Operation::Reply => 2,
        });
        // Sender MacAddr
        result.extend(&self.sender_hw.0);
        // Sender Ipv4
        result.extend(&self.sender_ip.0);
        // Target MacAddr
        result.extend(&self.target_hw.0);
        // Target Ipv4
        result.extend(&self.target_ip.0);
        // Return
        result
    }

    pub fn is_request(&self) -> bool {
        self.operation == Operation::Request
    }

    pub fn to_reply(mut self, mac: MacAddr, ip: Ipv4Addr) -> Self {
        assert!(self.is_request());

        self.operation = Operation::Reply;
        self.target_hw = self.sender_hw;
        self.target_ip = self.sender_ip;
        self.sender_hw = mac;
        self.sender_ip = ip;

        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum Operation {
    Request,
    Reply,
}
impl Operation {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes[0] == 0);
        match bytes[1] {
            1 => Self::Request,
            2 => Self::Reply,
            other => panic!("Unknown ARP operation {}", other),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse() {
        let example: Vec<u8> = vec![
            0, 1, 8, 0, 6, 4, 0, 1, 1, 2, 3, 4, 5, 6, 10, 0, 2, 2, 0, 0, 0, 0, 0, 0, 10, 0, 2, 15,
        ];

        let packet = Packet::from_bytes(&example);

        assert_eq!(packet, Packet {
            ptype: EtherType::Ipv4,
            operation: Operation::Request,
            sender_hw: MacAddr::from_bytes(&[1, 2, 3, 4, 5, 6]),
            sender_ip: Ipv4Addr::from_bytes(&[10, 0, 2, 2]),
            target_hw: MacAddr::from_bytes(&[0, 0, 0, 0, 0, 0]),
            target_ip: Ipv4Addr::from_bytes(&[10, 0, 2, 15]),
        });

        assert_eq!(packet.to_bytes(), example);
    }
}
