use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

pub mod protocol;

bitflags::bitflags! {
    #[derive(Default)]
    pub struct SubscriptionFlags: u64 {
        /// Topic filter is used as a prefix
        const PREFIX    = (1 << 0);
        /// Subscription is reliable, see ipc docs for more info
        const RELIABLE  = (1 << 1);
        /// First process sending to this subscription is marked
        /// as it's corresponding pipe pair. No messages from other
        /// processes will be accepted. If the sender is terminated
        /// all operations to this socket return an error.
        ///
        /// This can be used to release server resources if caller gets terminated,
        /// without needing to implement two-way communication.
        ///
        /// PIPE subscriptions are always EXACT and RELIABLE.
        const PIPE      = (1 << 2) | Self::PREFIX.bits | Self::RELIABLE.bits;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct SubscriptionId(u64);
impl SubscriptionId {
    pub const fn from_u64(v: u64) -> Self {
        Self(v)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Used to acknowledge a reliable message
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct AcknowledgeId(u64);
impl AcknowledgeId {
    pub fn from_u64(v: u64) -> Self {
        Self(v)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Message {
    /// Topic this message was sent to
    pub topic: String,
    /// The actual data on this message
    pub data: Vec<u8>,
    /// In case of reliable message, this is used to acknowledge the message.
    /// If this is none for a reliable message, then it's either:
    /// * sent by the kernel, and does not require an acknowledgement
    /// * sent as a reply, and does not require an acknowledgement
    pub ack_id: Option<AcknowledgeId>,
}
impl Message {
    pub fn needs_response(&self) -> bool {
        self.ack_id.is_some()
    }
}
