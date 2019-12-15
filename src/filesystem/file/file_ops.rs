use alloc::prelude::v1::*;
use hashbrown::HashSet;

use d7abi::process::{Error as ProcessError, ProcessId, ProcessResult};

use crate::multitasking::{ExplicitEventId, Scheduler, WaitFor};

use super::super::{error::*, path::Path, FileClientId, VirtualFS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leafness {
    /// Leaf node
    Leaf,
    /// Branch node managed by an attachment
    Branch,
    /// Internal branching node, that does not
    /// use normal ReadBranch protocol, but
    /// transmits internal ids instead
    InternalBranch,
}

/// Operations on an opened file (from perspective of the owner)
#[allow(unused_variables)]
pub trait FileOps: Send {
    /// Can this file has children in the filesystem.
    /// This check must not fail.
    /// Non-leaf nodes MUST conform to `ReadBranch` protocol.
    fn leafness(&self) -> Leafness;

    /// Pull some bytes from this source into the buffer, returning how many bytes were read.
    fn read(&mut self, fc: FileClientId, buf: &mut [u8]) -> IoResult<usize>;

    /// Returns `WaitFor::None` if this file is ready for reading,
    /// and the wait condition otherwise.
    fn read_waiting_for(&mut self, fc: FileClientId) -> WaitFor;

    /// Write a buffer into file, returning how many bytes were written
    ///
    /// If not implemented, causes `fs_readonly` error.
    fn write(&mut self, fc: FileClientId, buf: &[u8]) -> IoResult<usize> {
        IoResult::Code(ErrorCode::fs_readonly)
    }

    /// Allows device to perform some initialization when a new fc is opened.
    ///
    /// If not implemented, does nothing.
    fn open(&mut self, fc: FileClientId) -> IoResult<()> {
        IoResult::Success(())
    }

    /// Allows releasing resources when a fc is closed.
    /// This function must not fail.
    ///
    /// If not implemented, does nothing.
    fn close(&mut self, fc: FileClientId) {}

    /// Allows releasing resource when an instance is destroyed,
    /// e.g. when process is killed on death of the owner
    ///
    /// It can also be used to trigger some events on close,
    /// such as signaling waiting processes.
    ///
    /// If not implemented, does nothing.
    fn destroy(&mut self) -> Trigger {
        Trigger::empty()
    }

    /// Verify that all writes have reached their destination.
    ///
    /// If not implemented, does nothing.
    fn synchronize(&mut self, fc: FileClientId) -> IoResult<()> {
        IoResult::Success(())
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
    fn control(&mut self, fc: FileClientId, function: u64) -> IoResult<()> {
        IoResult::Code(ErrorCode::fs_unknown_control_function)
    }
}

#[derive(Debug, Clone, Default)]
#[must_use]
pub struct Trigger {
    trigger_events: HashSet<ExplicitEventId>,
    kill_processes: HashSet<ProcessId>,
}
impl Trigger {
    pub fn run(self, sched: &mut Scheduler, vfs: &mut VirtualFS) {
        // Send signals
        for event_id in self.trigger_events {
            sched.on_explicit_event(event_id);
        }

        // Kill processes
        for pid in self.kill_processes {
            sched.terminate(
                vfs,
                pid,
                ProcessResult::Failed(ProcessError::ChainedTermination),
            );
        }
    }

    pub fn empty() -> Self {
        Self {
            trigger_events: HashSet::new(),
            kill_processes: HashSet::new(),
        }
    }

    pub fn events(trigger_events: HashSet<ExplicitEventId>) -> Self {
        Self {
            trigger_events,
            ..Default::default()
        }
    }

    pub fn kill_processes(kill_processes: HashSet<ProcessId>) -> Self {
        Self {
            kill_processes,
            ..Default::default()
        }
    }

    pub fn kill_process(pid: ProcessId) -> Self {
        let mut kill_processes = HashSet::new();
        kill_processes.insert(pid);
        Self {
            kill_processes,
            ..Default::default()
        }
    }
}
