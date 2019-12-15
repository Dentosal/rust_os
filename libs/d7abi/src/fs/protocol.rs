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
pub mod attachment {
    use serde::{Serialize, Deserialize};
    use alloc::prelude::v1::*;
    use hashbrown::HashMap;

    use crate::fs::FileDescriptor;
    use crate::process::ProcessId;

    /// When manager process reads from or writes to an attachment,
    /// the contents are wrapped in `Message`
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Message {
        /// Sender process
        pub sender_pid: ProcessId,
        /// Sender identifier, unique per-process.
        /// Currently implemented as file descriptor,
        /// but this could be hashed to improve security.
        pub sender_f: u64, // TODO: Rename
        /// Type of the request
        pub type_: FileOperationType,
        /// The actual data, if any
        pub data: Vec<u8>,
    }
    impl Message {
        /// Converts request message to a reply by replacing the data
        pub fn into_reply(mut self, new_data: Vec<u8>) -> Self {
            self.data = new_data;
            self
        }
    }

    /// Currently open and close are not supported, but I think
    /// at least close must be implemented at some point
    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    pub enum FileOperationType {
        Read,
        ReadWaitingFor,
        Write,
        Control,
    }

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