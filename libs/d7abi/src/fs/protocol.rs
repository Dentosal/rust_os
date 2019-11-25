//! Data transfer protocol definitions.
//! The kernel encodes these using Serde with Pinecone serializer.

use serde::{Serialize, Deserialize};
use alloc::prelude::v1::*;

/// Branch ("directory") contents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadBranch {
    pub items: Vec<String>
}

/// Interface types for `fs_attach` system call
pub mod attach {
    use serde::{Serialize, Deserialize};
    use alloc::prelude::v1::*;
    use hashbrown::HashMap;

    use crate::fs::FileDescriptor;

    /// How branches ("directories") return their contents.
    /// The process MUST NOT return any data if the later
    /// reads would block, but must block on the first read
    /// call until they are ready.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ReadBranch {
        /// File descriptors here must be ones created
        /// by `fs_attach` system call.
        pub items: HashMap<String, FileDescriptor>
    }
}