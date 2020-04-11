use core::convert::TryFrom;
use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};

/// https://en.wikipedia.org/wiki/EtherType#Examples
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    TryFromPrimitive,
    Deserialize,
    Serialize,
)]
#[repr(u16)]
pub enum EtherType {
    Ipv4 = 0x0800,
    ARP = 0x0806,
    WakeOnLan = 0x0842,
    SLPP = 0x8102,
    Ipv6 = 0x86dd,
    EthernetFlowControl = 0x8808,
    EthernetSlowProtocol = 0x8809,
}
impl EtherType {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let n = u16::from_be_bytes([bytes[0], bytes[1]]);
        Self::try_from(n).unwrap_or_else(|_| panic!("Unknwn EtherType {:04x}", n))
    }

    pub fn to_bytes(self) -> [u8; 2] {
        u16::to_be_bytes(self as u16)
    }
}
