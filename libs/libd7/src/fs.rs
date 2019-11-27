use alloc::prelude::v1::*;

use d7abi::fs::{FileDescriptor, protocol};

use crate::syscall::{self, SyscallResult};

const IO_BUFFER_SIZE: usize = 1024;

pub fn list_dir(path: &str) -> SyscallResult<Vec<String>> {
    let fd: FileDescriptor = syscall::fs_open(path)?;
    let mut result = Vec::new();
    let mut buffer = [0u8; IO_BUFFER_SIZE];
    loop {
        // ReadBranch protocol guarantees that if any data at all
        // is read, the whole read operation will not block after
        // that. This means that error here is not an issue.
        let count = syscall::fd_read(fd, &mut buffer)?;
        if count == 0 {
            // EOF
            break;
        }
        result.extend(buffer.iter());
    }

    let branch: protocol::ReadBranch = pinecone::from_bytes(&result).unwrap();
    Ok(branch.items)
}