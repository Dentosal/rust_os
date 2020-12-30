use alloc::prelude::v1::*;
use core::fmt;
use core::ops::{Deref, DerefMut};
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use crate::multitasking::ExplicitEventId;

use super::attachment::Attachment;
use super::file::{CloseAction, FileOps};
use super::result::*;
use super::{FileClientId, Path};

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
pub struct Leaf {
    /// Contents
    pub(super) data: LeafData,
    /// Open file descriptors pointing to this node
    pub(super) fc_refcount: u64,
}
impl Leaf {
    pub fn new(obj: Box<dyn FileOps>) -> Self {
        Self {
            data: LeafData::FileObject(obj),
            fc_refcount: 0,
        }
    }

    pub fn new_attachment(manager: FileClientId, is_leaf: bool) -> Self {
        Self {
            data: LeafData::Attachment(Attachment::new(manager, is_leaf)),
            fc_refcount: 0,
        }
    }

    /// Increases reference count
    pub fn inc_ref(&mut self) {
        self.fc_refcount += 1;
    }

    /// Decreases reference count. If refcount hits zero,
    /// returns `false` to inform the caller that this node
    /// should be deleted
    #[must_use]
    pub fn dec_ref(&mut self) -> bool {
        assert_ne!(self.fc_refcount, 0, "close: fd refcount zero");
        self.fc_refcount -= 1;
        self.fc_refcount > 0
    }
}

/// Node contents
pub enum LeafData {
    FileObject(Box<dyn FileOps>),
    Attachment(Attachment),
}
impl fmt::Debug for LeafData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileObject(_) => write!(f, "LeafData::FileObject(...)"),
            Self::Attachment(_) => write!(f, "LeafData::Attachment(...)"),
        }
    }
}
