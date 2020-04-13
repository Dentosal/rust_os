use serde::{Deserialize, Serialize};

pub mod protocol;

/// VFS node metadata.
/// `Copy` is required here as kernel copies it into the process memory.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(C)]
pub struct FileInfo {
    /// Leaf nodes cannot have children.
    /// Non-leaf nodes use directory contents protocol.
    pub is_leaf: bool,
}

/// VFS process-unique file descriptor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct FileDescriptor(u64);
impl FileDescriptor {
    #![allow(clippy::missing_safety_doc)]

    /// Creates new file descriptor from raw u64
    pub fn from_u64(raw: u64) -> Self {
        Self(raw)
    }

    /// Obtains raw integer value of this fd
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Next file descriptor
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}
