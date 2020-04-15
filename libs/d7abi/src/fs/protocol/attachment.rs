//! Interface types for `fs_attach` system call

use alloc::prelude::v1::*;
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use crate::fs::FileDescriptor;
use crate::process::ProcessId;

/// Sender identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sender {
    /// Sender process, None for kernel
    pub pid: Option<ProcessId>,
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
    pub operation: RequestFileOperation,
}
impl Request {
    /// Converts request message to a reply by replacing the data
    pub fn response(&self, operation: ResponseFileOperation) -> Response {
        Response {
            sender: self.sender,
            operation,
        }
    }
}

/// When manager process reads from or writes to an attachment,
/// the contents are wrapped in `Request`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Sender of the corresponding `Request` identifier
    pub sender: Sender,
    /// Response data
    pub operation: ResponseFileOperation,
}

/// Currently open, control and waiting_for are not supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestFileOperation {
    /// Read n bytes
    Read(u64),
    /// Write bytes
    Write(Vec<u8>),
    /// Close the file
    Close,
}

/// Response to request operation
/// There is no response for close
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseFileOperation {
    Read(Vec<u8>),
    Write(u64),
}


/// How branches ("directories") return their contents to the kernel.
/// The process MUST NOT return any data if the later reads would block,
/// but must block on the first read call until they are ready.
/// Note that this is not same as `ReadBranch` protocol,
/// which is used to return folder contents to processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadAttachmentBranch {
    /// File descriptors here must be ones created
    /// by `fs_attach` system call.
    pub items: HashMap<String, FileDescriptor>,
}
