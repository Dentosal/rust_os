use d7abi::fs::FileDescriptor;
use d7abi::process::ProcessResult;

pub use d7abi::process::ProcessId;

use crate::syscall::{self, SyscallResult};

/// A safe wrapper for processes
#[derive(Debug)]
pub struct Process {
    pub fd: FileDescriptor,
    pub pid: ProcessId,
}
impl Process {
    pub fn spawn(path: &str) -> SyscallResult<Self> {
        let fd = syscall::fs_exec(path)?;
        Ok(Self {
            fd,
            pid: syscall::fd_get_pid(fd)?,
        })
    }

    pub fn pid(&self) -> ProcessId {
        self.pid
    }

    pub fn wait(self) -> SyscallResult<ProcessResult> {
        let mut buffer = [0; 9];
        let bytes = syscall::fd_read(self.fd, &mut buffer)?;
        debug_assert!(bytes == buffer.len());
        Ok(pinecone::from_bytes(&buffer[..bytes]).unwrap())
    }
}
