use alloc::vec::Vec;

#[derive(Debug)]
pub struct Packet {
    pub header: Header,
    pub payload: Vec<u8>,
}
impl Packet {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let header = Header::from_bytes(&bytes[..8]);
        let payload = bytes[8..(header.length as usize)].to_vec();
        Self { header, payload }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = self.header.to_bytes().to_vec();
        result.extend(self.payload);
        assert!(result.len() == (self.header.length as usize));
        result
    }
}

#[derive(Debug)]
pub struct Header {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}
impl Header {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let src_port = u16::from_be_bytes([bytes[0], bytes[1]]);
        let dst_port = u16::from_be_bytes([bytes[2], bytes[3]]);
        let length = u16::from_be_bytes([bytes[4], bytes[5]]);
        let checksum = u16::from_be_bytes([bytes[6], bytes[7]]);
        Self {
            src_port,
            dst_port,
            length,
            checksum,
        }
    }

    pub fn to_bytes(&self) -> [u8; 8] {
        let mut result = [0u8; 8];
        result[0..2].copy_from_slice(&u16::to_be_bytes(self.src_port));
        result[2..4].copy_from_slice(&u16::to_be_bytes(self.dst_port));
        result[4..6].copy_from_slice(&u16::to_be_bytes(self.length));
        result[6..8].copy_from_slice(&u16::to_be_bytes(self.checksum));
        result
    }
}
