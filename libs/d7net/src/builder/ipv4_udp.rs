use alloc::vec::Vec;

use crate::ipv4;
use crate::udp;
use crate::{IpProtocol, Ipv4Addr};
use crate::checksum::inet_checksum;

pub struct Builder {
    pub ipv4_header: ipv4::Header,
    pub udp_header: udp::Header,
    pub payload: Vec<u8>,
}
impl Builder {
    /// TODO: fragmentation support
    pub fn new(
        src_ip: Ipv4Addr, dst_ip: Ipv4Addr, src_port: u16, dst_port: u16, payload: Vec<u8>,
    ) -> Self {
        Self {
            ipv4_header: ipv4::Header {
                dscp_and_ecn: 0,
                payload_len: 0,
                identification: 0,
                flags_and_frament: 0,
                ttl: 64,
                protocol: IpProtocol::UDP,
                src_ip,
                dst_ip,
            },
            udp_header: udp::Header {
                src_port,
                dst_port,
                length: 0,   // Filled in later
                checksum: 0, // Filled in later
            },
            payload,
        }
    }

    pub fn build(mut self) -> Vec<u8> {
        self.udp_header.length = (8 + self.payload.len()) as u16;
        let mut cksm_buf = Vec::new();
        cksm_buf.extend(&self.ipv4_header.src_ip.0);
        cksm_buf.extend(&self.ipv4_header.dst_ip.0);
        cksm_buf.push(0);
        cksm_buf.push(IpProtocol::UDP as u8);
        cksm_buf.extend(&u16::to_be_bytes(self.udp_header.length));
        cksm_buf.extend(&self.udp_header.to_bytes());
        cksm_buf.extend(&self.payload);
        self.udp_header.checksum = inet_checksum(&cksm_buf);

        let mut result = Vec::new();
        let udp_header = self.udp_header.to_bytes();
        result.extend(&self.ipv4_header.to_bytes(udp_header.len() + self.payload.len()));
        result.extend(&udp_header);
        result.extend(&self.payload);
        result
    }
}
