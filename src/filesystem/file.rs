use d7abi::FileDescriptor;

use super::path::Path;
use super::FsResult;

/// Operations on an opened file from mount owners perspective
pub trait FileOps: Send {
    /// Pull some bytes from this source into the buffer, returning how many bytes were read
    fn read(&mut self, fd: FileDescriptor, buf: &mut [u8]) -> FsResult<usize>;

    /// Write a buffer into file, returning how many bytes were written
    fn write(&mut self, fd: FileDescriptor, buf: &[u8]) -> FsResult<usize>;

    /// Verify that all writes have reached their destination
    fn synchronize(&mut self, fd: FileDescriptor) -> FsResult<()>;

    /// Request device-specific control information transfer.
    /// The device can decide whether this "selects a channel",
    /// or will it swtch back to normal io mode after completition.
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
    fn control(&mut self, fd: FileDescriptor, function: u64) -> FsResult<()>;
}
