use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{EtherType, MacAddr};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}
impl Frame {
    pub fn from_bytes(input: &[u8]) -> Self {
        Self {
            header: FrameHeader::from_bytes(&input[0..14]),
            payload: input[14..].to_vec(),
        }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend(&self.header.to_bytes());
        result.extend(&self.payload);
        // Return
        result
    }
}

/// https://en.wikipedia.org/wiki/Ethernet_frame#Frame_%E2%80%93_data_link_layer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct FrameHeader {
    pub dst_mac: MacAddr,
    pub src_mac: MacAddr,
    pub ethertype: EtherType,
}
impl FrameHeader {
    pub fn from_bytes(input: &[u8]) -> Self {
        Self {
            dst_mac: MacAddr::from_bytes(&input[0..6]),
            src_mac: MacAddr::from_bytes(&input[6..12]),
            ethertype: EtherType::from_bytes(&input[12..14]),
        }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend(&self.dst_mac.0);
        result.extend(&self.src_mac.0);
        result.extend(&self.ethertype.to_bytes());
        // Return
        result
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_frame_header() {
        let example: Vec<u8> = vec![
            255, 255, 255, 255, 255, 255, // Dst MAC
            1, 2, 3, 4, 5, 6, // Src MAC
            8, 6, // EtherType
        ];

        let frame_header = FrameHeader::from_bytes(&example);
        assert_eq!(frame_header, FrameHeader {
            dst_mac: MacAddr::from_bytes(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]),
            src_mac: MacAddr::from_bytes(&[1, 2, 3, 4, 5, 6]),
            ethertype: EtherType::ARP,
        });

        assert_eq!(frame_header.to_bytes(), example);
    }
}
