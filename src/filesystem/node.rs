use alloc::prelude::v1::*;
use core::fmt;
use core::ops::{Deref, DerefMut};
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use d7abi::fs::FileInfo;

use super::error::*;
use super::file::{FileOps, Leafness};
use super::FileClientId;
use super::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    pub fn new(dev: Box<dyn FileOps>) -> Self {
        Self {
            data: NodeData(dev),
            fd_refcount: 0,
        }
    }

    pub fn leafness(&self) -> Leafness {
        self.data.leafness()
    }

    pub fn fileinfo(&self) -> FileInfo {
        FileInfo {
            is_leaf: self.leafness() == Leafness::Leaf,
        }
    }

    /// Calls handler and (on success) increases reference count
    pub fn open(&mut self, fd: FileClientId) -> IoResult<()> {
        self.data.open(fd)?;
        self.fd_refcount += 1;
        Ok(())
    }

    /// Calls handler and decreases reference count. Always succeeds.
    pub fn close(&mut self, fd: FileClientId) {
        assert_ne!(self.fd_refcount, 0, "close: fd refcount zero");
        self.fd_refcount -= 1;
        self.data.close(fd);
    }

    /// Reads slice of data from this node,
    /// and returns how many bytes were read
    pub fn read(&mut self, fd: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        self.data.read(fd, buf)
    }

    /// Writes slice of data from this node,
    /// and returns how many bytes were written
    pub fn write(&mut self, fd: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        self.data.write(fd, buf)
    }
}

/// Node contents
pub struct NodeData(Box<dyn FileOps>);
impl fmt::Debug for NodeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}
impl Deref for NodeData {
    type Target = dyn FileOps;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
impl DerefMut for NodeData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut()
    }
}
