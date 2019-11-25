use alloc::prelude::v1::*;

use super::super::{error::*, path::Path, FileClientId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leafness {
    /// Leaf node
    Leaf,
    /// Branch node
    Branch,
    /// Internal branching node, that does not
    /// use normal ReadBranch protocol, but
    /// transmits internal ids instead
    InternalBranch,
}

/// Operations on an opened file from mount owners perspective
pub trait FileOps: Send {
    /// Can this file has children in the filesystem.
    /// This check must not fail.
    /// Non-leaf nodes MUST conform to `ReadBranch` protocol.
    fn leafness(&self) -> Leafness;

    /// Pull some bytes from this source into the buffer, returning how many bytes were read.
    fn read(&mut self, fd: FileClientId, buf: &mut [u8]) -> IoResult<usize>;

    /// Write a buffer into file, returning how many bytes were written
    ///
    /// If not implemented, causes `fs_readonly` error.
    fn write(&mut self, fd: FileClientId, buf: &[u8]) -> IoResult<usize> {
        Err(IoError::Code(ErrorCode::fs_readonly))
    }

    /// Allows device to perform some initialization when a new fd is opened.
    ///
    /// If not implemented, does nothing.
    fn open(&mut self, fd: FileClientId) -> IoResult<()> {
        Ok(())
    }

    /// Allows releasing resources when a fd is closed.
    /// This function must not fail.
    ///
    /// If not implemented, does nothing.
    fn close(&mut self, fd: FileClientId) {}

    /// Verify that all writes have reached their destination.
    ///
    /// If not implemented, does nothing.
    fn synchronize(&mut self, fd: FileClientId) -> IoResult<()> {
        Ok(())
    }

    /// Request device-specific control information transfer.
    /// The device can decide whether this "selects a channel",
    /// or will it swtch back to normal io mode after completition.
    ///
    /// If not implemented, will return `fs_unknown_control_function` error.
    ///
    /// ```ignore
    /// file.write(b"1234")?;
    /// // Seek to byte in index 1
    /// file.control(FILE_SEEK_ABS);
    /// file.write(&1u64.to_le_bytes());
    /// // Overwrite some bytes
    /// file.write(b"321")?;
    /// // File now contains 1321
    /// ```
    fn control(&mut self, fd: FileClientId, function: u64) -> IoResult<()> {
        Err(IoError::Code(ErrorCode::fs_unknown_control_function))
    }
}
