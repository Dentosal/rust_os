use serde::{Deserialize, Serialize};

pub mod protocol;

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
