use alloc::prelude::v1::*;

use crate::multitasking::{
    process::{ProcessId, ProcessResult},
    ExplicitEventId, WaitFor,
};

use super::super::{error::*, path::Path, FileClientId};

use super::{FileOps, Leafness, Trigger};

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
    /// File descriptor for the file
    file_fc: FileClientId,
    /// Result of the process, if it's completed
    result: Option<ProcessResult>,
}
impl ProcessFile {
    pub fn new(pid: ProcessId, file_fc: FileClientId) -> Self {
        Self {
            pid,
            file_fc,
            result: None,
        }
    }
}
impl FileOps for ProcessFile {
    fn leafness(&self) -> Leafness {
        Leafness::Leaf
    }

    /// Blocks until the process is complete, and the returns the result
    fn read(&mut self, fc: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        assert_ne!(fc.process, Some(self.pid), "Process read self");
        if let Some(result) = &self.result {
            let data = pinecone::to_vec(&result).unwrap();
            assert!(
                data.len() <= buf.len(),
                "Read process: buffer not large enough (required: {} <= {})",
                data.len(),
                buf.len()
            );
            buf[..data.len()].copy_from_slice(&data);
            IoResult::Success(data.len())
        } else {
            IoResult::RepeatAfter(WaitFor::Process(self.pid))
        }
    }

    fn read_waiting_for(&mut self, fc: FileClientId) -> WaitFor {
        assert_ne!(fc.process, Some(self.pid), "Process read self");
        if self.result.is_some() {
            WaitFor::None
        } else {
            WaitFor::Process(self.pid)
        }
    }

    fn write(&mut self, fc: FileClientId, buf: &[u8]) -> IoResult<usize> {
        if fc.is_kernel() {
            // Kernel writes set result code
            self.result = Some(pinecone::from_bytes(buf).unwrap());
            IoResult::Success(buf.len())
        } else {
            // Process writes are not allowed yet
            IoResult::Code(ErrorCode::fs_readonly)
        }
    }

    /// When process file is destroyed, e.g. on owner process death,
    /// the process must be killed
    fn destroy(&mut self) -> Trigger {
        Trigger::kill_process(self.pid)
    }
}
