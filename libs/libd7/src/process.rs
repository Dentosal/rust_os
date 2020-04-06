use d7abi::fs::FileDescriptor;
use d7abi::process::ProcessResult;

pub use d7abi::process::ProcessId;

use crate::syscall::{self, SyscallResult};

/// A safe wrapper for processes
#[derive(Debug)]
pub struct Process {
    pub fd: FileDescriptor,
}
impl Process {
    pub fn spawn(path: &str) -> SyscallResult<Self> {
        Ok(Self { fd: syscall::fs_exec(path)? })
    }

    pub fn wait(self) -> SyscallResult<ProcessResult> {
        let mut buffer = [0; 9];
        let bytes = syscall::fd_read(self.fd, &mut buffer)?;
        debug_assert!(bytes == buffer.len());
        Ok(pinecone::from_bytes(&buffer[..bytes]).unwrap())
    }
}