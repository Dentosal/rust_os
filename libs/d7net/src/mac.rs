use alloc::prelude::v1::*;
use core::fmt;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacAddr(pub [u8; 6]);

impl MacAddr {
    pub const ZERO: Self = Self([0, 0, 0, 0, 0, 0]);
    pub const BROADCAST: Self = Self([0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);

    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() == 6);
        let mut data = [0; 6];
        data.copy_from_slice(bytes);
        MacAddr(data)
    }
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<_> = self.0.iter().map(|c| format!("{:02x}", c)).collect();
        write!(f, "MacAddr({})", parts.join(":"))
    }
}
