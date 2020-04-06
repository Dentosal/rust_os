//! Data transfer protocol definitions.
//! The kernel encodes these using Serde with Pinecone serializer.

use alloc::prelude::v1::*;
use serde::{Deserialize, Serialize};

/// Branch ("directory") contents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadBranch {
    pub items: Vec<String>,
}

/// Interface types for `fs_attach` system call
pub mod attachment {
    use alloc::prelude::v1::*;
    use hashbrown::HashMap;
    use serde::{Deserialize, Serialize};

    use crate::fs::FileDescriptor;
    use crate::process::ProcessId;

    /// Sender identifier
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct Sender {
        /// Sender process
        pub pid: ProcessId,
        /// Sender identifier, unique per-process.
        /// Currently implemented as file descriptor,
        /// but this could be hashed to improve security.
        pub f: u64, // TODO: Rename
    }

    /// When manager process reads from or writes to an attachment,
    /// the contents are wrapped in `Request`
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Request {
        /// Sender identifier
        pub sender: Sender,
        /// Contents of the request
        pub data: FileOperation,
    }
    impl Request {
        /// Converts request message to a reply by replacing the data
        pub fn response(&self, data: Vec<u8>) -> Response {
            Response {
                sender: self.sender,
                data,
            }
        }
    }

    /// Currently open and close are not supported, but I think
    /// at least close must be implemented at some point
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum FileOperation {
        /// Read n bytes
        Read(u64),
        /// Query if this file is ready for reading,
        /// and the wait condition otherwise.
        ReadWaitingFor(u64),
        /// Write bytes
        Write(Vec<u8>),
        /// Control request
        Control(u64),
    }

    /// When manager process reads from or writes to an attachment,
    /// the contents are wrapped in `Request`
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Response {
        /// Sender of the corresponding `Request` identifier
        pub sender: Sender,
        /// Response data
        pub data: Vec<u8>,
    }

    /// How branches ("directories") return their contents.
    /// The process MUST NOT return any data if the later
    /// reads would block, but must block on the first read
    /// call until they are ready.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ReadBranch {
        /// File descriptors here must be ones created
        /// by `fs_attach` system call.
        pub items: HashMap<String, FileDescriptor>,
    }
}
