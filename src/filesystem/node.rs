use alloc::prelude::v1::*;
use core::fmt;
use core::ops::{Deref, DerefMut};
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use d7abi::fs::FileInfo;

use crate::multitasking::ExplicitEventId;

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
    /// Parent node id, None for root
    pub(super) parent: Option<NodeId>,
    /// Contents
    pub(super) data: NodeData,
    /// Open file descriptors pointing to this node
    pub(super) fd_refcount: u64,
}
impl Node {
    pub fn new(parent: Option<NodeId>, dev: Box<dyn FileOps>) -> Self {
        Self {
            parent,
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
        self.inc_ref();
        IoResult::Success(())
    }

    /// Calls handler that always always succeeds, amd then
    /// decreases reference count. If refcount hits zero,
    /// returns `false` to inform the caller that this node
    /// should be deleted.
    /// Also returns a vec of events to trigger
    #[must_use]
    pub fn close(&mut self, fd: FileClientId) -> bool {
        assert_ne!(self.fd_refcount, 0, "close: fd refcount zero");
        self.data.close(fd);
        self.dec_ref()
    }

    /// Increases reference count
    pub fn inc_ref(&mut self) {
        self.fd_refcount += 1;
    }

    /// Decreases reference count. If refcount hits zero,
    /// returns `false` to inform the caller that this node
    /// should be deleted
    #[must_use]
    pub fn dec_ref(&mut self) -> bool {
        assert_ne!(self.fd_refcount, 0, "close: fd refcount zero");
        self.fd_refcount -= 1;
        self.fd_refcount > 0
    }

    /// Reads slice of data from this node,
    /// and returns how many bytes were read
    pub fn read(&mut self, fd: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        self.data.read(fd, buf)
    }

    /// Writes slice of data from this node,
    /// and returns how many bytes were written
    pub fn write(&mut self, fd: FileClientId, buf: &[u8]) -> IoResult<usize> {
        self.data.write(fd, buf)
    }
}

/// Node contents
pub struct NodeData(Box<dyn FileOps>);
impl fmt::Debug for NodeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeData(...)")
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