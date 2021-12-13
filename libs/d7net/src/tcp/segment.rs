use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

const INITIAL_WINDOW_SIZE: u16 = 8760;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Segment {
    header: SegmentHeader,
    payload: Vec<u8>,
}
impl Segment {
    pub fn from_bytes(input: &[u8]) -> Self {
        let header = SegmentHeader::from_bytes(input);
        Self {
            payload: input[header.offset..].to_vec(),
            header,
        }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = Vec::new();
        todo!();
        result
    }
}

/// https://en.wikipedia.org/wiki/Transmission_Control_Protocol#TCP_segment_structure
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SegmentHeader {
    src_port: u16,
    dst_port: u16,
    sequence: u32,
    ack_number: u32,
    flags: SegmentFlags,
    window_size: u16,
    options: SegmentOptions,
    offset: usize,
}
impl SegmentHeader {
    const OFFSET_NO_OPTIONS: usize = 20;

    pub fn from_bytes(input: &[u8]) -> Self {
        let offset_and_flags = u16::from_be_bytes([input[12], input[13]]);
        let offset = (offset_and_flags >> 12) as usize * 4;
        let flags = SegmentFlags::from_bits_truncate(offset_and_flags & 0x1f);
        let option_bytes = &input[Self::OFFSET_NO_OPTIONS..offset];

        Self {
            src_port: u16::from_be_bytes([input[0], input[1]]),
            dst_port: u16::from_be_bytes([input[2], input[3]]),
            sequence: u32::from_be_bytes([input[4], input[5], input[6], input[7]]),
            ack_number: u32::from_be_bytes([input[8], input[9], input[10], input[11]]),
            flags,
            window_size: u16::from_be_bytes([input[14], input[15]]),
            options: SegmentOptions::from_bytes(option_bytes),
            offset,
        }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = Vec::new();
        todo!();
        result
    }

    /// SYN flag set, no ACK
    pub fn is_initialization(&self) -> bool {
        self.flags.contains(SegmentFlags::SYN) && (!self.flags.contains(SegmentFlags::ACK))
    }

    /// SYN & ACK flags set
    pub fn is_initialization_reply(&self) -> bool {
        self.flags.contains(SegmentFlags::SYN) && self.flags.contains(SegmentFlags::ACK)
    }

    /// SYN-ACK reply to initialization (SYN)
    pub fn reply_to_initialization(self, next_sequence: u32) -> Self {
        Self {
            src_port: self.dst_port,
            dst_port: self.src_port,
            sequence: next_sequence,
            ack_number: self.sequence.wrapping_add(1),
            flags: SegmentFlags::SYN | SegmentFlags::ACK,
            window_size: INITIAL_WINDOW_SIZE,
            options: SegmentOptions::empty(),
            offset: Self::OFFSET_NO_OPTIONS,
        }
    }

    /// ACK reply to initialization (SYN-ACK)
    pub fn reply_to_synack(self, next_sequence: u32) -> Self {
        Self {
            src_port: self.dst_port,
            dst_port: self.src_port,
            sequence: next_sequence,
            ack_number: self.sequence.wrapping_add(1),
            flags: SegmentFlags::ACK,
            window_size: INITIAL_WINDOW_SIZE,
            options: SegmentOptions::empty(),
            offset: Self::OFFSET_NO_OPTIONS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct SegmentOptions {
    /// SYN-only option
    segemnt_max_size: Option<u16>,
}
impl SegmentOptions {
    pub fn empty() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn from_bytes(mut input: &[u8]) -> Self {
        let mut result = Self {
            ..Default::default()
        };
        while !input.is_empty() {
            match input[0] {
                0 => break, // End of options list
                1 => {
                    // NOP padding
                    input = &input[1..];
                },
                2 => {
                    // Maximum segment size
                    assert_eq!(input[1], 4);
                    result.segemnt_max_size = Some(u16::from_be_bytes([input[2], input[3]]));
                    input = &input[4..];
                },
                other => {
                    // Unsupported TCP option
                    panic!("Unsupported TCP option {:?}", input)
                },
            }
        }
        result
    }
}

bitflags::bitflags! {
    #[derive(Deserialize, Serialize)]
    pub struct SegmentFlags: u16 {
        const FIN     = 1 << 0;
        const SYN     = 1 << 1;
        const RST     = 1 << 2;
        const PSH     = 1 << 3;
        const ACK     = 1 << 4;
        const URG     = 1 << 5;
        const ECE     = 1 << 6;
        const CWR     = 1 << 7;
        const NS      = 1 << 8;
    }
}
