use num_enum::TryFromPrimitive;

#[derive(Debug, TryFromPrimitive)]
#[allow(non_camel_case_types)]
#[repr(u64)]
pub enum SyscallNumber {
    exit = 0x00,
    get_pid = 0x01,
    debug_print = 0x02,
    mem_set_size = 0x03,
    fs_fileinfo = 0x30,
    fs_create = 0x31,
    fs_open = 0x32,
    fd_close = 0x40,
    fd_read = 0x41,
    fd_write = 0x42,
    fd_synchronize = 0x43,
    fd_control = 0x44,
    sched_yield = 0x50,
    sched_sleep_ns = 0x51,
}

#[derive(Debug, TryFromPrimitive)]
#[allow(non_camel_case_types)]
#[repr(u64)]
pub enum SyscallErrorCode {
    unknown = 0,
}

/// VFS node metadata.
/// `Copy` is required here as kernel copies it into the process memory.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct FileInfo {
    /// Is this a special device managed by the kenrel
    pub is_special: bool,
    /// Mount id, if this is a mount point
    pub mount_id: Option<u64>,
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
