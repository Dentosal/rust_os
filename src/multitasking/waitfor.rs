use alloc::prelude::v1::*;

use d7abi::fs::FileDescriptor;
use d7time::Instant;

use crate::multitasking::ProcessId;

use super::queues::Queues;

/// Instructions for scheduling a process
#[derive(Debug, Clone)]
pub enum WaitFor {
    /// Run again on the next free slot
    None,
    /// Run after specified moment
    Time(Instant),
    /// Process completed
    Process(ProcessId),
    /// First of multiple wait conditions.
    /// Should never contain `None`.
    FirstOf(Vec<WaitFor>),
}
impl WaitFor {
    /// Minimize based on conditions.
    /// Used to make sure that process that is already
    /// completed is not waited for.
    pub fn reduce(self, qs: &Queues, current: ProcessId) -> Self {
        use WaitFor::*;
        match self {
            Process(p) if current != p && !qs.process_exists(p) => {
                panic!("SKIP PROC {} {}", p, current);
                None
            },
            other => other,
        }
    }
}
