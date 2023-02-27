//! Commands sent to other cpu cores. Note that kernel panic is sent using a separate IPI.

use alloc::vec::Vec;
use d7abi::process::ProcessId;
use spin::Mutex;

use crate::{
    driver::ioapic::send_ipi,
    multitasking::{ProcessSwitch, SCHEDULER},
};

use super::{data::PerCpu, ProcessorId};

static QUEUES: PerCpu<Mutex<Vec<Command>>> = PerCpu::new();

/// Send a command to a CPU
pub fn send(to: ProcessorId, command: Command) {
    let mut queue = QUEUES.for_cpu(to).lock();
    queue.push(command);
    send_ipi(to.acpi_id(), 0xda, false);
}

/// This core received a notification about command
pub fn on_receive() -> ProcessSwitch {
    let mut queue = QUEUES.current_cpu().lock();
    queue.sort();
    queue.dedup();

    for command in queue.drain(..) {
        log::debug!("Processing IPI command {command:?}");
        match command {
            Command::KernelTlbFlush => todo!("Kernel TLB flush command"),
            Command::StopTerminatedProcess(_) => todo!("StopTerminatedProcess"),
            Command::ProcessAvailable => {
                let mut sched = SCHEDULER.lock();
                return sched.continuation(false);
            },
        }
    }

    ProcessSwitch::Continue
}

/// Ordering is used to process some types of messages first.
/// All messages are idempotent, and duplicates are skipped.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Command {
    /// Perform a TLB shootdown.
    KernelTlbFlush,
    /// A process has been terminated in the scheduler.
    /// Stop the process if it's still running.
    StopTerminatedProcess(ProcessId),
    /// The CPU was idle and there is a process available for running.
    ProcessAvailable,
}
