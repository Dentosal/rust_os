//! Data transfer protocol definitions.
//! The kernel encodes these using Serde with Pinecone serializer.

use alloc::{vec::Vec, string::String};
use serde::{Deserialize, Serialize};

pub mod attachment;

/// Branch ("directory") contents
/// Note that this is not same as `ReadAttachmentBranch` protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadBranch {
    pub items: Vec<String>,
}
