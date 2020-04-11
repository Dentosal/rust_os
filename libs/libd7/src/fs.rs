use alloc::prelude::v1::*;

use d7abi::fs::{protocol, FileDescriptor};

use crate::syscall::{self, SyscallResult};

pub fn list_dir(path: &str) -> SyscallResult<Vec<String>> {
    // TODO: verify that path is a leaf

    let result = read(path)?;
    let branch: protocol::ReadBranch = pinecone::from_bytes(&result).unwrap();
    Ok(branch.items)
}

#[derive(Debug)]
pub struct File {
    pub fd: FileDescriptor,
}
impl File {
    pub fn open(path: &str) -> SyscallResult<Self> {
        Ok(Self {
            fd: syscall::fs_open(path)?,
        })
    }

    /// Zero on EOF
    pub fn read(&self, buf: &mut [u8]) -> SyscallResult<usize> {
        syscall::fd_read(self.fd, buf)
    }

    pub fn write(&self, buf: &[u8]) -> SyscallResult<usize> {
        syscall::fd_write(self.fd, buf)
    }

    pub fn write_all(&self, buf: &[u8]) -> SyscallResult<()> {
        let mut data = buf;
        while !data.is_empty() {
            let count = syscall::fd_write(self.fd, data)?;
            data = &data[count..];
        }
        Ok(())
    }

    pub fn close(self) -> SyscallResult<()> {
        syscall::fd_close(self.fd)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = syscall::fd_close(self.fd);
    }
}

pub fn read(path: &str) -> SyscallResult<Vec<u8>> {
    const BUF_SIZE: usize = 1024;
    let mut buffer = [0u8; BUF_SIZE];
    let mut result = Vec::new();
    let file = File::open(path)?;
    loop {
        let count = file.read(&mut buffer)?;
        result.extend(&buffer[..count]);
        if count < BUF_SIZE {
            break;
        }
    }
    Ok(result)
}
