use crate::multitasking::{
    process::{ProcessId, ProcessResult},
    WaitFor,
};

use super::super::{error::*, path::Path, FileClientId};

use super::{FileOps, Leafness};

/// # Process
/// Reading a process blocks until the process is
/// completed, and then returns its exit status.
///
/// Writing to a process is currently not implmented,
/// but it could be purposed to sending signals, or
/// just simply terminating the process.
#[derive(Debug, Clone)]
pub struct ProcessFile {
    /// Id of the process
    pid: ProcessId,
    /// Result of the process, if it's completed
    result: Option<ProcessResult>,
}
impl FileOps for ProcessFile {
    fn leafness(&self) -> Leafness {
        Leafness::Leaf
    }

    /// Blocks until the process is complete, and the returns the result
    fn read(&mut self, _fd: FileClientId, _buf: &mut [u8]) -> IoResult<usize> {
        if let Some(result) = &self.result {
            unimplemented!("Process {} read: Write to buffer {:?}", self.pid, result);
        } else {
            rprintln!("PROC WAIT {}", self.pid);
            Err(IoError::RepeatAfter(WaitFor::Process(self.pid)))
        }
    }

    fn write(&mut self, _fd: FileClientId, buf: &[u8]) -> IoResult<usize> {
        unimplemented!("Process write") // TODO
    }

    fn synchronize(&mut self, _fd: FileClientId) -> IoResult<()> {
        unimplemented!("Process sync") // TODO
    }

    fn control(&mut self, _fd: FileClientId, _: u64) -> IoResult<()> {
        unimplemented!("Process ctrl") // TODO
    }
}
