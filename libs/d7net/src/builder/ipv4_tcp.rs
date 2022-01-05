use alloc::vec::Vec;

use crate::checksum::inet_checksum;
use crate::ipv4;
use crate::tcp;
use crate::{IpProtocol, Ipv4Addr};

#[derive(Debug)]
pub struct Builder {
    pub ipv4_header: ipv4::Header,
    pub tcp_header: tcp::SegmentHeader,
    pub payload: Vec<u8>,
}
impl Builder {
    /// TODO: fragmentation support
    pub fn new(
        src_ip: Ipv4Addr, dst_ip: Ipv4Addr, src_port: u16, dst_port: u16, sequence: u32,
        ack_number: u32, window_size: u16, flags: tcp::SegmentFlags, payload: Vec<u8>,
    ) -> Self {
        Self {
            ipv4_header: ipv4::Header {
                dscp_and_ecn: 0,
                payload_len: 0,
                identification: 0,
                flags_and_frament: 0,
                ttl: 64,
                protocol: IpProtocol::TCP,
                src_ip,
                dst_ip,
            },
            tcp_header: tcp::SegmentHeader {
                src_port,
                dst_port,
                sequence,
                ack_number,
                flags,
                window_size,
                options: tcp::SegmentOptions::empty(),
                checksum: 0,
                offset: tcp::SegmentHeader::OFFSET_NO_OPTIONS,
            },
            payload,
        }
    }

    pub fn build(mut self) -> Vec<u8> {
        self.tcp_header.checksum = 0;
        let mut cksm_buf = Vec::new();
        cksm_buf.extend(&self.ipv4_header.src_ip.0);
        cksm_buf.extend(&self.ipv4_header.dst_ip.0);
        cksm_buf.push(0);
        cksm_buf.push(IpProtocol::TCP as u8);
        cksm_buf.extend(u16::to_be_bytes(
            (self.tcp_header.to_bytes().len() + self.payload.len()) as u16,
        ));
        cksm_buf.extend(&self.tcp_header.to_bytes());
        cksm_buf.extend(&self.payload);
        self.tcp_header.checksum = inet_checksum(&cksm_buf);

        let mut result = Vec::new();
        let tcp_header = self.tcp_header.to_bytes();
        result.extend(
            &self
                .ipv4_header
                .to_bytes(tcp_header.len() + self.payload.len()),
        );
        result.extend(&tcp_header);
        result.extend(&self.payload);
        result
    }
}
