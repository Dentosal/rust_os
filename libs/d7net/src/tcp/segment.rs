use alloc::vec::Vec;

pub use tcpstate::SegmentFlags;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub header: SegmentHeader,
    pub payload: Vec<u8>,
}
impl Segment {
    pub fn from_bytes(input: &[u8]) -> Self {
        let header = SegmentHeader::from_bytes(input);
        Self {
            payload: input[header.offset..].to_vec(),
            header,
        }
    }
}

/// https://en.wikipedia.org/wiki/Transmission_Control_Protocol#TCP_segment_structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub sequence: u32,
    pub ack_number: u32,
    pub flags: SegmentFlags,
    pub window_size: u16,
    pub options: SegmentOptions,
    pub checksum: u16,
    pub offset: usize,
}
impl SegmentHeader {
    pub const OFFSET_NO_OPTIONS: usize = 20;

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
            checksum: 0, // TODO
            offset,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend(&u16::to_be_bytes(self.src_port));
        result.extend(&u16::to_be_bytes(self.dst_port));
        result.extend(&u32::to_be_bytes(self.sequence));
        result.extend(&u32::to_be_bytes(self.ack_number));
        // TODO: data offset
        let data_offset = ((self.offset / 4) as u16) << 12;
        let b = data_offset | self.flags.bits();
        result.extend(&u16::to_be_bytes(b));
        result.extend(&u16::to_be_bytes(self.window_size));
        result.extend(&u16::to_be_bytes(self.checksum));
        result.extend(&u16::to_be_bytes(0));
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

    /// ACK flag set, no other change flags
    pub fn is_normal(&self) -> bool {
        self.flags.contains(SegmentFlags::ACK)
            && !(self.flags.contains(SegmentFlags::SYN) || self.flags.contains(SegmentFlags::FIN))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
                    panic!("Unsupported TCP option {:?}", other)
                },
            }
        }
        result
    }
}
