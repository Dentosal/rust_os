//! Special files implemented in the kernel VFS for performance and simplicity

use d7abi::FileDescriptor;

use super::file::FileOps;
use super::{FsError, FsResult};

/// `/dev/null`
pub struct NullDevice;
impl FileOps for NullDevice {
    /// Immediately provides EOF
    fn read(&mut self, _fd: FileDescriptor, _buf: &mut [u8]) -> FsResult<usize> {
        Ok(0)
    }

    /// Discards all data
    fn write(&mut self, _fd: FileDescriptor, buf: &[u8]) -> FsResult<usize> {
        Ok(buf.len())
    }

    /// No synchronization required
    fn synchronize(&mut self, _fd: FileDescriptor) -> FsResult<()> {
        Ok(())
    }

    /// No controls available
    fn control(&mut self, _fd: FileDescriptor, _: u64) -> FsResult<()> {
        Err(FsError::ControlFunction)
    }
}

/// `/dev/zero`
pub struct ZeroDevice;
impl FileOps for ZeroDevice {
    /// Zeroes the buffer
    fn read(&mut self, _fd: FileDescriptor, buf: &mut [u8]) -> FsResult<usize> {
        for i in 0..buf.len() {
            buf[i] = 0;
        }
        Ok(buf.len())
    }

    /// No data will be written
    fn write(&mut self, _fd: FileDescriptor, _buf: &[u8]) -> FsResult<usize> {
        Ok(0)
    }

    /// No synchronization required
    fn synchronize(&mut self, _fd: FileDescriptor) -> FsResult<()> {
        Ok(())
    }

    /// No controls available
    fn control(&mut self, _fd: FileDescriptor, _: u64) -> FsResult<()> {
        Err(FsError::ControlFunction)
    }
}
