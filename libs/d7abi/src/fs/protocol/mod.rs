//! Data transfer protocol definitions.
//! The kernel encodes these using Serde with Pinecone serializer.

use alloc::prelude::v1::*;
use serde::{Deserialize, Serialize};

pub mod attachment;
pub mod console;
pub mod network;

/// Branch ("directory") contents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadBranch {
    pub items: Vec<String>,
}
