
/// VFS node metadata.
/// `Copy` is required here as kernel copies it into the process memory.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct FileInfo {
    /// Leaf nodes cannot have children.
    /// Non-leaf nodes use directory contents protocol.
    pub is_leaf: bool,
}

/// VFS file descriptor
/// # Safety
/// Almost all operations on file descriptors are unsafe,
/// as they can be used to obtain invalid file descriptors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct FileDescriptor(u64);
impl FileDescriptor {
    #![allow(clippy::missing_safety_doc)]

    /// Creates new file descriptor from raw u64
    pub unsafe fn from_u64(raw: u64) -> Self {
        Self(raw)
    }

    /// Obtains raw integer value of this fd
    pub unsafe fn as_u64(self) -> u64 {
        self.0
    }

    /// Next file descriptor
    pub unsafe fn next(self) -> Self {
        Self(self.0 + 1)
    }
}
