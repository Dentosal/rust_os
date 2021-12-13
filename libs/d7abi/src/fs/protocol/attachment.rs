//! Interface types for `fs_attach` system call

use alloc::{vec::Vec, string::String};
use serde::{Deserialize, Serialize};

use crate::process::ProcessId;
use crate::SyscallErrorCode;

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
    /// Suffix, i.e. the path after the attachment point (for branch nodes)
    pub suffix: Option<String>,
    /// Contents of the request
    pub operation: RequestFileOperation,
}
impl Request {
    /// Converts request message to a reply by replacing the data
    pub fn response(&self, operation: ResponseFileOperation) -> Response {
        Response {
            sender: self.sender,
            suffix: self.suffix.clone(),
            operation,
        }
    }

    /// Converts request message to a reply by replacing the data
    pub fn respond<F>(&self, f: F) -> Response
    where F: FnOnce(&Self) -> ResponseFileOperation {
        let r = f(self);
        self.response(r)
    }
}

/// When manager process reads from or writes to an attachment,
/// the contents are wrapped in `Request`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Sender of the corresponding `Request` identifier
    pub sender: Sender,
    /// Suffix, i.e. the path after the attachment point (for branch nodes)
    pub suffix: Option<String>,
    /// Response data
    pub operation: ResponseFileOperation,
}

/// Currently control and waiting_for are not supported
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
    Error(SyscallErrorCode),
    Read(Vec<u8>),
    Write(u64),
}
