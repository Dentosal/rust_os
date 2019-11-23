use alloc::prelude::v1::*;
use core::fmt;
use hashbrown::HashMap;

use d7abi::{FileDescriptor, FileInfo};

use crate::multitasking::{
    process::{ProcessId, ProcessResult},
    WaitFor,
};

use super::error::*;
use super::file::FileOps;
use super::path::Path;

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

    pub fn fileinfo(&self) -> FileInfo {
        match &self.data {
            NodeData::Branch(_) => FileInfo { is_leaf: false },
            NodeData::Special(_) => FileInfo { is_leaf: true },
            NodeData::Process(_) => FileInfo { is_leaf: true },
            NodeData::Mount(mt) => FileInfo {
                is_leaf: mt.is_leaf,
            },
        }
    }

    pub fn get_child(&self, name: &str) -> IoResult<NodeId> {
        match &self.data {
            NodeData::Branch(b) => b
                .children
                .get(name)
                .copied()
                .ok_or(IoError::Code(ErrorCode::fs_node_not_found)),
            NodeData::Special(_) => Err(IoError::Code(ErrorCode::fs_node_is_leaf)),
            NodeData::Process(_) => Err(IoError::Code(ErrorCode::fs_node_is_leaf)),
            NodeData::Mount(_) => unimplemented!("Mount"),
        }
    }

    pub fn has_child(&self, name: &str) -> IoResult<bool> {
        match &self.data {
            NodeData::Branch(b) => Ok(b.children.contains_key(name)),
            NodeData::Special(_) => Err(IoError::Code(ErrorCode::fs_node_is_leaf)),
            NodeData::Process(_) => Err(IoError::Code(ErrorCode::fs_node_is_leaf)),
            NodeData::Mount(_) => unimplemented!("Mount"),
        }
    }

    pub fn add_child(&mut self, id: NodeId, name: &str) -> IoResult<()> {
        match &mut self.data {
            NodeData::Branch(b) => {
                b.children.insert(name.to_owned(), id);
                Ok(())
            },
            NodeData::Special(_) => Err(IoError::Code(ErrorCode::fs_node_is_leaf)),
            NodeData::Process(_) => Err(IoError::Code(ErrorCode::fs_node_is_leaf)),
            NodeData::Mount(_) => unimplemented!("Mount"),
        }
    }

    /// Increases reference count
    pub fn open(&mut self) -> IoResult<()> {
        self.fd_refcount += 1;
        Ok(())
    }

    /// Decreases reference count. Always succeeds.
    pub fn close(&mut self) {
        assert_ne!(self.fd_refcount, 0, "close: fd refcount zero");
        self.fd_refcount -= 1;
    }

    /// Reads slice of data from this node,
    /// and returns how many bytes were read
    pub fn read(&mut self, fd: FileDescriptor, buf: &mut [u8]) -> IoResult<usize> {
        match &mut self.data {
            NodeData::Branch(b) => b.read(fd, buf),
            NodeData::Special(s) => (*s).read(fd, buf),
            NodeData::Process(p) => p.read(fd, buf),
            NodeData::Mount(m) => unimplemented!("READ MOUNT"),
        }
    }
}

/// Node contents
pub enum NodeData {
    /// Chilren for this node
    Branch(VirtualBranch),
    /// Special files only implemented in the kernel.
    /// These are always leaf nodes.
    Special(Box<dyn FileOps>),
    /// # Process
    /// Reading a process blocks until the process is
    /// completed, and then returns its exit status.
    ///
    /// Writing to a process is currently not implmented,
    /// but it could be purposed to sending signals, or
    /// just simply terminating the process.
    Process(ProcessFile),
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
pub struct ProcessFile {
    /// Id of the process
    pid: ProcessId,
    /// Result of the process, if it's completed
    result: Option<ProcessResult>,
}
impl FileOps for ProcessFile {
    /// Blocks until the process is complete, and the returns the result
    fn read(&mut self, _fd: FileDescriptor, _buf: &mut [u8]) -> IoResult<usize> {
        if let Some(result) = &self.result {
            unimplemented!("Process {} read: Write to buffer {:?}", self.pid, result);
        } else {
            rprintln!("PROC WAIT {}", self.pid);
            Err(IoError::RepeatAfter(WaitFor::Process(self.pid)))
        }
    }

    fn write(&mut self, _fd: FileDescriptor, buf: &[u8]) -> IoResult<usize> {
        unimplemented!("Process write") // TODO
    }

    fn synchronize(&mut self, _fd: FileDescriptor) -> IoResult<()> {
        unimplemented!("Process sync") // TODO
    }

    fn control(&mut self, _fd: FileDescriptor, _: u64) -> IoResult<()> {
        unimplemented!("Process ctrl") // TODO
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
    fn read(&mut self, fd: FileDescriptor, buf: &mut [u8]) -> IoResult<usize> {
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

    fn write(&mut self, _fd: FileDescriptor, buf: &[u8]) -> IoResult<usize> {
        unimplemented!("VB write") // TODO
    }

    fn synchronize(&mut self, _fd: FileDescriptor) -> IoResult<()> {
        unimplemented!("VB sync") // TODO
    }

    fn control(&mut self, _fd: FileDescriptor, _: u64) -> IoResult<()> {
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
    /// Leafness is a static property of a mount,
    /// the controlling process cannot change this
    is_leaf: bool,
}
