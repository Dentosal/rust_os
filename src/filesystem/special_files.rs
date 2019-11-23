//! Special files implemented in the kernel VFS for performance and simplicity

use d7abi::FileDescriptor;

use super::error::*;
use super::file::FileOps;

/// `/dev/null`
pub struct NullDevice;
impl FileOps for NullDevice {
    /// Immediately provides EOF
    fn read(&mut self, _fd: FileDescriptor, _buf: &mut [u8]) -> IoResult<usize> {
        Ok(0)
    }

    /// Discards all data
    fn write(&mut self, _fd: FileDescriptor, buf: &[u8]) -> IoResult<usize> {
        Ok(buf.len())
    }

    /// No synchronization required
    fn synchronize(&mut self, _fd: FileDescriptor) -> IoResult<()> {
        Ok(())
    }

    /// No controls available
    fn control(&mut self, _fd: FileDescriptor, _: u64) -> IoResult<()> {
        Err(IoError::Code(ErrorCode::fs_unknown_control_function))
    }
}

/// `/dev/zero`
pub struct ZeroDevice;
impl FileOps for ZeroDevice {
    /// Zeroes the buffer
    fn read(&mut self, _fd: FileDescriptor, buf: &mut [u8]) -> IoResult<usize> {
        for i in 0..buf.len() {
            buf[i] = 0;
        }
        Ok(buf.len())
    }

    /// No data will be written
    fn write(&mut self, _fd: FileDescriptor, _buf: &[u8]) -> IoResult<usize> {
        Ok(0)
    }

    /// No synchronization required
    fn synchronize(&mut self, _fd: FileDescriptor) -> IoResult<()> {
        Ok(())
    }

    /// No controls available
    fn control(&mut self, _fd: FileDescriptor, _: u64) -> IoResult<()> {
        Err(IoError::Code(ErrorCode::fs_unknown_control_function))
    }
}

/// `/dev/test`
/// Testing device for fs features
pub struct TestDevice {
    pub rounds: u8,
}
impl FileOps for TestDevice {
    fn read(&mut self, _fd: FileDescriptor, buf: &mut [u8]) -> IoResult<usize> {
        use crate::multitasking::WaitFor;
        use crate::time::SYSCLOCK;
        use core::time::Duration;

        rprintln!("/dev/test: READ");
        if self.rounds == 0 {
            rprintln!("/dev/test: DONE!");
            return Err(IoError::Code(ErrorCode::fs_unknown_control_function));
        }

        let after = SYSCLOCK.now() + Duration::from_millis(1000);
        self.rounds -= 1;
        Err(IoError::RepeatAfter(WaitFor::Time(after)))
    }

    fn write(&mut self, _fd: FileDescriptor, _buf: &[u8]) -> IoResult<usize> {
        Ok(0)
    }

    fn synchronize(&mut self, _fd: FileDescriptor) -> IoResult<()> {
        Ok(())
    }

    fn control(&mut self, _fd: FileDescriptor, _: u64) -> IoResult<()> {
        Err(IoError::Code(ErrorCode::fs_unknown_control_function))
    }
}
