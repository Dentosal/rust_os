use alloc::prelude::v1::*;

pub use d7abi::process::{ProcessId, ProcessResult};

use crate::syscall::{self, SyscallResult};
use crate::ipc;

/// A safe wrapper for a process
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Process {
    pid: ProcessId,
}
impl Process {
    pub fn spawn(path: &str) -> SyscallResult<Self> {
        let image: Vec<u8> = ipc::request("initrd/read", path)?;
        let pid = syscall::exec(&image)?;
        Ok(Process { pid })
    }

    pub fn pid(&self) -> ProcessId {
        self.pid
    }
}
