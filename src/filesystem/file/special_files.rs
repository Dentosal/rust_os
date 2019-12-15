//! Special files implemented in the kernel VFS for performance and simplicity

use crate::multitasking::WaitFor;

use super::super::{error::*, path::Path, FileClientId};
use super::{FileOps, Leafness};

/// `/dev/null`
pub struct NullDevice;
impl FileOps for NullDevice {
    fn leafness(&self) -> Leafness {
        Leafness::Leaf
    }

    /// Immediately provides EOF
    fn read(&mut self, _fd: FileClientId, _buf: &mut [u8]) -> IoResult<usize> {
        IoResult::Success(0)
    }

    fn read_waiting_for(&mut self, fc: FileClientId) -> WaitFor {
        WaitFor::None
    }

    /// Discards all data
    fn write(&mut self, _fd: FileClientId, buf: &[u8]) -> IoResult<usize> {
        IoResult::Success(buf.len())
    }
}

/// `/dev/zero`
pub struct ZeroDevice;
impl FileOps for ZeroDevice {
    fn leafness(&self) -> Leafness {
        Leafness::Leaf
    }

    /// Zeroes the buffer
    fn read(&mut self, _fd: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        for i in 0..buf.len() {
            buf[i] = 0;
        }
        IoResult::Success(buf.len())
    }

    fn read_waiting_for(&mut self, fc: FileClientId) -> WaitFor {
        WaitFor::None
    }

    /// No data will be written
    fn write(&mut self, _fd: FileClientId, _buf: &[u8]) -> IoResult<usize> {
        IoResult::Success(0)
    }
}

/// `/dev/test`
/// Testing device for fs features
pub struct TestDevice {
    pub rounds: u8,
}
impl FileOps for TestDevice {
    fn leafness(&self) -> Leafness {
        Leafness::Leaf
    }

    fn read(&mut self, _fd: FileClientId, _buf: &mut [u8]) -> IoResult<usize> {
        use crate::multitasking::WaitFor;
        use crate::time::SYSCLOCK;
        use core::time::Duration;

        rprintln!("/dev/test: READ");
        if self.rounds == 0 {
            rprintln!("/dev/test: DONE!");
            return IoResult::Code(ErrorCode::fs_unknown_control_function);
        }

        let after1 = SYSCLOCK.now() + Duration::from_millis(1000);
        let after2 = SYSCLOCK.now() + Duration::from_millis(1000);
        self.rounds -= 1;
        panic!("TESTDEV");
        IoResult::RepeatAfter(WaitFor::FirstOf(vec![
            WaitFor::Time(after1),
            WaitFor::Time(after2),
        ]))
    }

    fn read_waiting_for(&mut self, fc: FileClientId) -> WaitFor {
        WaitFor::None
    }

    fn write(&mut self, _fd: FileClientId, _buf: &[u8]) -> IoResult<usize> {
        IoResult::Success(0)
    }
}
