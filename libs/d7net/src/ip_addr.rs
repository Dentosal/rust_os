use alloc::prelude::v1::*;
use core::fmt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct Ipv4Addr(pub [u8; 4]);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct Ipv6Addr(pub [u8; 16]);

impl Ipv4Addr {
    pub const ZERO: Self = Self([0, 0, 0, 0]);
    pub const LOCALHOST: Self = Self([127, 0, 0, 1]);

    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() == 4);
        let mut data = [0; 4];
        data.copy_from_slice(bytes);
        Ipv4Addr(data)
    }
}

impl Ipv6Addr {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() == 16);
        let mut data = [0; 16];
        data.copy_from_slice(bytes);
        Ipv6Addr(data)
    }
}

impl fmt::Debug for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<_> = self.0.iter().map(|c| format!("{}", c)).collect();
        write!(f, "Ipv4Addr({})", parts.join("."))
    }
}

impl fmt::Debug for Ipv6Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<_> = self.0.iter().map(|c| format!("{:02x}", c)).collect();
        write!(f, "Ipv6Addr({})", parts.join(":"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum IpAddr {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}
impl IpAddr {
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        match bytes.len() {
            4 => Self::V4(Ipv4Addr::from_bytes(bytes)),
            16 => Self::V6(Ipv6Addr::from_bytes(bytes)),
            _ => panic!("Invalid ip address byte count"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SocketAddr {
    pub host: IpAddr,
    pub port: u16,
}
