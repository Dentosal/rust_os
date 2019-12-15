use d7abi::fs::FileDescriptor;

pub use d7abi::fs::protocol::attachment::{Message, FileOperationType};

use crate::syscall::{self, SyscallResult};

/// A safe wrapper for a branch attachment point
#[derive(Debug)]
pub struct Branch {
    pub fd: FileDescriptor,
}
impl Branch {
    pub fn new(path: &str) -> SyscallResult<Self> {
        Ok(Self { fd: syscall::fs_attach(path, false)? })
    }
}

/// A safe wrapper for a leaf attachment point
#[derive(Debug)]
pub struct Leaf {
    pub fd: FileDescriptor,
}
impl Leaf {
    pub fn new(path: &str) -> SyscallResult<Self> {
        Ok(Self { fd: syscall::fs_attach(path, true)? })
    }

    /// Receive next request
    pub fn next_request(&self) -> SyscallResult<Message> {
        let mut buffer = [0u8; 32];
        let count = syscall::fd_read(self.fd, &mut buffer)?;
        Ok(pinecone::from_bytes(&buffer[..count]).unwrap())
    }

    /// Reply to a received request
    pub fn reply(&self, message: Message) -> SyscallResult<()> {
        let buffer = pinecone::to_vec(&message).unwrap();
        let count = syscall::fd_write(self.fd, &buffer)?;
        assert_eq!(buffer.len(), count, "TODO: Multipart writes");
        Ok(())
    }
}