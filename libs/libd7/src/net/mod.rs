#![allow(unreachable_code)] // TODO remove this

use serde::{Deserialize, Serialize};

pub use d7net;

pub mod tcp;
// pub mod udp;

pub use d7net::SocketAddr;

/// Used to acknowledge a reliable message
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct SocketId(u64);
impl SocketId {
    pub fn from_u64(v: u64) -> Self {
        Self(v)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}
