use alloc::prelude::v1::*;
use d7abi::fs::FileDescriptor;
use hashbrown::HashMap;

pub use d7abi::fs::protocol::attachment::*;
use d7abi::fs::protocol::ReadBranch;

use crate::syscall::{self, SyscallResult};

/// A safe wrapper for a static branch attachment point.
/// Static branch contains a list of members,
/// unlike normal branch that is rebuilt on every request.
/// Clients cannot write to staticbranch at all.
#[derive(Debug)]
pub struct StaticBranch {
    pub inner: Branch,
    pub items: HashMap<String, FileDescriptor>,
}
impl StaticBranch {
    pub fn new(path: &str) -> SyscallResult<Self> {
        Ok(Self {
            inner: Branch::new(path)?,
            items: HashMap::new(),
        })
    }

    /// Receive next request and reply
    pub fn process_one(&self) -> SyscallResult<()> {
        let r = self.inner.next_request()?;
        match r.operation {
            RequestFileOperation::Read(n) => {
                // TODO: handle kernel requests differently
                // let data = pinecone::to_vec(&ReadAttachmentBranch { items: self.items.clone() }).unwrap();

                let data = pinecone::to_vec(&ReadBranch {
                    items: self.items.keys().cloned().collect(),
                })
                .unwrap();

                assert!(data.len() as u64 <= n, "TODO: implement output buffering");
                self.inner
                    .reply(r.response(ResponseFileOperation::Read(data)))?;
            }
            RequestFileOperation::Close => {}
            _ => panic!("Unsupported operation to a static branch ({:?})", r),
        }
        Ok(())
    }

    pub fn add_branch(&mut self, name: &str) -> SyscallResult<Branch> {
        let new_branch = Branch::new_anonymous()?;
        self.items.insert(name.to_owned(), new_branch.fd);
        Ok(new_branch)
    }

    pub fn add_leaf(&mut self, name: &str) -> SyscallResult<Leaf> {
        let new_leaf = Leaf::new_anonymous()?;
        self.items.insert(name.to_owned(), new_leaf.fd);
        Ok(new_leaf)
    }
}

/// A safe wrapper for a branch attachment point
#[derive(Debug)]
pub struct Branch {
    pub fd: FileDescriptor,
}
impl Branch {
    pub fn new(path: &str) -> SyscallResult<Self> {
        Ok(Self {
            fd: syscall::fs_attach(path, false)?,
        })
    }

    pub fn new_anonymous() -> SyscallResult<Self> {
        Self::new("")
    }

    /// Receive next request
    pub fn next_request(&self) -> SyscallResult<Request> {
        let mut buffer = [0u8; 32]; // TODO: is 32 always enough?
        let count = syscall::fd_read(self.fd, &mut buffer)?;
        Ok(pinecone::from_bytes(&buffer[..count]).unwrap())
    }

    /// Reply to a received request
    pub fn reply(&self, response: Response) -> SyscallResult<()> {
        let buffer = pinecone::to_vec(&response).unwrap();
        let count = syscall::fd_write(self.fd, &buffer)?;
        assert_eq!(buffer.len(), count, "TODO: Multipart writes");
        Ok(())
    }
}

/// A safe wrapper for a leaf attachment point
#[derive(Debug)]
pub struct Leaf {
    pub fd: FileDescriptor,
}
impl Leaf {
    pub fn new(path: &str) -> SyscallResult<Self> {
        Ok(Self {
            fd: syscall::fs_attach(path, true)?,
        })
    }

    pub fn new_anonymous() -> SyscallResult<Self> {
        Self::new("")
    }

    /// Receive next request
    pub fn next_request(&self) -> SyscallResult<Request> {
        let mut buffer = [0u8; 32]; // TODO: is 32 always enough?
        let count = syscall::fd_read(self.fd, &mut buffer)?;
        Ok(pinecone::from_bytes(&buffer[..count]).unwrap())
    }

    /// Reply to a received request
    pub fn reply(&self, response: Response) -> SyscallResult<()> {
        let buffer = pinecone::to_vec(&response).unwrap();
        let count = syscall::fd_write(self.fd, &buffer)?;
        assert_eq!(buffer.len(), count, "TODO: Multipart writes");
        Ok(())
    }
}
