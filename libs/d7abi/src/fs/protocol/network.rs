use serde::{Deserialize, Serialize};
use alloc::prelude::v1::*;

use d7time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedPacket {
    /// Packet contents
    pub packet: Vec<u8>,
    /// Timestamp
    pub timestamp: Instant,
}
