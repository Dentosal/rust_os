use alloc::vec::Vec;
use d7abi::fs::FileDescriptor;
use hashbrown::HashMap;

pub use d7abi::fs::protocol::attachment::*;
use d7abi::fs::protocol::ReadBranch;

use crate::syscall::{self, SyscallResult};

/// A safe wrapper for am attachment point
#[derive(Debug)]
pub struct Attachment {
    pub fd: FileDescriptor,
}
impl Attachment {
    #[must_use]
    pub fn new_leaf(path: &str) -> SyscallResult<Self> {
        assert!(!path.is_empty());
        Ok(Self {
            fd: syscall::fs_attach(path, true)?,
        })
    }

    #[must_use]
    pub fn new_branch(path: &str) -> SyscallResult<Self> {
        assert!(!path.is_empty());
        Ok(Self {
            fd: syscall::fs_attach(path, false)?,
        })
    }

    pub fn buffered(self) -> BufferedAttachment {
        BufferedAttachment {
            inner: self,
            output_buffer: HashMap::new(),
        }
    }

    /// Receive next request
    #[must_use]
    pub fn next_request(&self) -> SyscallResult<Request> {
        let mut buffer = [0u8; 1024]; // TODO: is 1024 always enough? not for long suffixes
        let count = syscall::fd_read(self.fd, &mut buffer)?;
        Ok(pinecone::from_bytes(&buffer[..count]).unwrap())
    }

    /// Reply to a received request
    #[must_use]
    pub fn reply(&self, response: Response) -> SyscallResult<()> {
        let buffer = pinecone::to_vec(&response).unwrap();
        let count = syscall::fd_write(self.fd, &buffer)?;
        assert_eq!(buffer.len(), count, "TODO: Multipart writes");
        Ok(())
    }
}

/// Attachment with a reply buffer used to store replies to read requests
#[derive(Debug)]
pub struct BufferedAttachment {
    pub inner: Attachment,
    output_buffer: HashMap<(Sender, Option<String>), ResponseFileOperation>,
}
impl BufferedAttachment {
    /// Receive next request
    #[must_use]
    pub fn next_request(&mut self) -> Option<SyscallResult<Request>> {
        // TODO: clean reply on close
        match self.inner.next_request() {
            Ok(rq) => {
                if let Some(buffered) = self.output_buffer.remove(&(rq.sender, rq.suffix.clone())) {
                    if let Err(e) = self.reply(rq.response(buffered)) {
                        Some(Err(e))
                    } else {
                        None
                    }
                } else {
                    Some(Ok(rq))
                }
            }
            Err(e) => Some(Err(e)),
        }
    }

    /// Reply to a received request
    #[must_use]
    pub fn reply(&self, response: Response) -> SyscallResult<()> {
        self.inner.reply(response)
    }

    pub fn buffer_reply(&mut self, response: Response) {
        self.output_buffer.insert((response.sender, response.suffix), response.operation);
    }
}
