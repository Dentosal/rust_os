use core::fmt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
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
        write!(
            f,
            "MacAddr({:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x})",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5],
        )
    }
}
