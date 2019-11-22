use core::fmt;

use alloc::prelude::v1::*;
use hashbrown::HashMap;

use d7abi::FileDescriptor;

use crate::multitasking::ProcessId;

use super::file::FileOps;
use super::path::Path;
use super::{FsError, FsResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NodeId(u64);
impl NodeId {
    pub(super) const fn first() -> Self {
        Self(0)
    }

    pub(super) const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// A node in the virtual filesystem tree.
/// Somewhat analogous to Unix inode.
#[derive(Debug)]
pub struct Node {
    /// Contents
    pub(super) data: NodeData,
    /// Open file descriptors pointing to this node
    pub(super) fd_refcount: u64,
}
impl Node {
    pub fn new_branch() -> Self {
        Self {
            data: NodeData::Branch(VirtualBranch::new()),
            fd_refcount: 0,
        }
    }

    pub fn new_special(dev: Box<dyn FileOps>) -> Self {
        Self {
            data: NodeData::Special(dev),
            fd_refcount: 0,
        }
    }

    pub fn fileinfo(&self) -> d7abi::FileInfo {
        match &self.data {
            NodeData::Branch(_) => d7abi::FileInfo {
                is_special: false,
                mount_id: None,
            },
            NodeData::Special(_) => d7abi::FileInfo {
                is_special: true,
                mount_id: None,
            },
            NodeData::Mount(target) => d7abi::FileInfo {
                is_special: false,
                mount_id: Some(target.mount_id),
            },
        }
    }

    pub fn get_child(&self, name: &str) -> FsResult<NodeId> {
        match &self.data {
            NodeData::Branch(b) => b.children.get(name).copied().ok_or(FsError::NodeNotFound),
            NodeData::Special(_) => Err(FsError::NodeNotFound),
            NodeData::Mount(_) => unimplemented!("Mount"),
        }
    }

    pub fn has_child(&self, name: &str) -> FsResult<bool> {
        match &self.data {
            NodeData::Branch(b) => Ok(b.children.contains_key(name)),
            NodeData::Special(_) => Ok(false),
            NodeData::Mount(_) => unimplemented!("Mount"),
        }
    }

    pub fn add_child(&mut self, id: NodeId, name: &str) -> FsResult<()> {
        match &mut self.data {
            NodeData::Branch(b) => {
                b.children.insert(name.to_owned(), id);
                Ok(())
            },
            NodeData::Special(_) => Err(FsError::NodeCreation),
            NodeData::Mount(_) => unimplemented!("Mount"),
        }
    }

    /// Increases reference count
    pub fn open(&mut self) -> FsResult<()> {
        self.fd_refcount += 1;
        Ok(())
    }

    /// Decreases reference count
    pub fn close(&mut self) -> FsResult<()> {
        assert_ne!(self.fd_refcount, 0, "close: fd refcount zero");
        self.fd_refcount -= 1;
        Ok(())
    }

    /// Reads slice of data from this node,
    /// and returns how many bytes were read
    pub fn read(&mut self, fd: FileDescriptor, buf: &mut [u8]) -> FsResult<usize> {
        match &mut self.data {
            NodeData::Branch(b) => b.read(fd, buf),
            NodeData::Special(s) => (*s).read(fd, buf),
            NodeData::Mount(m) => unimplemented!("READ MOUNT"),
        }
    }
}

/// Node contents
pub enum NodeData {
    /// Chilren for this node
    Branch(VirtualBranch),
    /// Special files only implemented in the kernel.
    Special(Box<dyn FileOps>),
    /// # Mount point
    /// This node and its contents are managed by a driver
    /// software. On branch nodes, the driver can provide
    /// child nodes that are used in addition to the ones
    /// described here.
    /// ## Caching
    /// In the future, this filesystem tree should be able
    /// to cache filesystem trees from drivers that indicate
    /// that the paths are allowed to be cached.
    /// ## Nesting mount points
    /// Nested mounts are allowed.
    /// The innermost mount point will receive all operations,
    /// and the relayed path is relative to the mount point.
    /// ## Unmounting
    /// Unlike Linux, where unmounting requires that all inner
    /// mounts are unmounted first, this implementation simply
    /// fabricates paths until the inner mount point.
    Mount(MountTarget),
}
impl fmt::Debug for NodeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeData::Special(_) => write!(f, "Special"),
            other => write!(f, "{:?}", other),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VirtualBranch {
    children: HashMap<String, NodeId>,
    /// Readers and their snapshots of data.
    /// Data is already formatted for reading, and is
    /// stored in reverse order for fast `pop` operations.
    /// Read bytes are removed from the buffer.
    readers: HashMap<FileDescriptor, Vec<u8>>,
}
impl VirtualBranch {
    pub fn new() -> Self {
        Self {
            children: HashMap::new(),
            readers: HashMap::new(),
        }
    }

    /// Formats contents for reading.
    /// Provides entries in arbitrary order.
    /// Each entry is prefix with little-endian u64,
    /// giving its length in bytes.
    fn format_contents(&self) -> Vec<u8> {
        let mut result = Vec::new();
        for name in self.children.keys() {
            let len = (name.len() as u64);
            for byte in name.bytes().rev() {
                result.push(byte);
            }
            for byte in len.to_le_bytes().iter().rev() {
                result.push(*byte);
            }
        }
        result
    }
}
impl FileOps for VirtualBranch {
    /// Provides next bytes from reader buffer.
    /// See `format_contents` for explanation of the format.
    fn read(&mut self, fd: FileDescriptor, buf: &mut [u8]) -> FsResult<usize> {
        if !self.readers.contains_key(&fd) {
            let content = self.format_contents();
            self.readers.insert(fd, content);
        }
        let reader_buf = self.readers.get_mut(&fd).unwrap();
        let mut count = 0;
        while count < buf.len() {
            if let Some(byte) = reader_buf.pop() {
                buf[count] = byte;
                count += 1;
            } else {
                break;
            }
        }
        Ok(count)
    }

    fn write(&mut self, _fd: FileDescriptor, buf: &[u8]) -> FsResult<usize> {
        unimplemented!("VB write") // TODO
    }

    fn synchronize(&mut self, _fd: FileDescriptor) -> FsResult<()> {
        unimplemented!("VB sync") // TODO
    }

    fn control(&mut self, _fd: FileDescriptor, _: u64) -> FsResult<()> {
        unimplemented!("VB ctrl") // TODO
    }
}
#[derive(Debug, Clone)]
pub struct MountTarget {
    /// Identifier of this mount point.
    /// Used by the managing process to differentiate
    /// multiple mount points.
    mount_id: u64,
    /// Process managing the mount point
    process_id: ProcessId,
}
