use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use hashbrown::HashMap;
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};

use crate::memory;
use crate::memory::phys::OutOfMemory;
use crate::multitasking::ExplicitEventId;
use crate::smp::sleep::ns_to_ticks;
use crate::time::BSPInstant;

use super::process::{Process, ProcessResult, ProcessSwitchInfo};
use super::queues::Queues;
use super::{ElfImage, ProcessId, WaitFor};

/// Time slice given to each process
const TIME_SLICE_NS: u64 = 100_000_000;

/// Smallest time that a process will be scheduled for exection
const MIN_EXEC_TIME_NS: u64 = TIME_SLICE_NS / 10;

/// Process switch an related alternatives
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
#[must_use]
pub enum ProcessSwitch {
    /// Keep the same process
    Continue,
    /// Go to idle state
    Idle,
    /// Switch to a different process
    Switch(ProcessSwitchInfo),
    /// Switch to a new process, by repeating
    /// a system call, and continuing after that.
    /// Syscall number and arguments must be stored in the process
    RepeatSyscall(ProcessSwitchInfo),
}

#[derive(Debug)]
pub struct Scheduler {
    /// Processes by id
    processes: HashMap<ProcessId, Process>,
    /// Queues for different types of scheduling
    queues: Queues,
    /// Id of the currently running process
    running: Option<ProcessId>,
    /// End of the timeslice of the currently running process
    running_timeslice_end: Option<BSPInstant>,
    /// Next available process id
    next_pid: ProcessId,
}
impl Scheduler {
    pub unsafe fn new() -> Self {
        Self {
            processes: HashMap::new(),
            queues: Queues::new(),
            running: None,
            running_timeslice_end: None,
            next_pid: ProcessId::first(),
        }
    }

    /// Get id of the current process
    pub fn get_running_pid(&self) -> Option<ProcessId> {
        self.running
    }

    /// Used for swapping out the process
    /// # Safety
    /// Caller must ensure that the process is given back using `give_back_process`
    pub unsafe fn take_process_by_id(&mut self, pid: ProcessId) -> Option<Process> {
        self.processes.remove(&pid)
    }

    /// Used for swapping out the process
    /// # Safety
    /// Only processes take with `take_process_by_id` must be used
    pub unsafe fn give_back_process(&mut self, process: Process) {
        self.processes.insert(process.id(), process);
    }

    pub fn process_by_id(&self, pid: ProcessId) -> Option<&Process> {
        self.processes.get(&pid)
    }

    pub fn process_by_id_mut(&mut self, pid: ProcessId) -> Option<&mut Process> {
        self.processes.get_mut(&pid)
    }

    /// Returns process count
    pub fn process_count(&self) -> u64 {
        self.processes.len() as u64
    }

    /// Returns list of all process ids
    pub fn process_ids(&self) -> Vec<ProcessId> {
        self.processes.keys().copied().collect()
    }

    /// Creates a new process, and returns its pid
    pub fn spawn(&mut self, args: &[String], elf: ElfImage) -> Result<ProcessId, OutOfMemory> {
        let pid = self.next_pid;
        self.next_pid = self.next_pid.next();
        let process = unsafe { Process::create(pid, args, elf)? };
        self.processes.insert(pid, process);
        self.queues.give(pid, WaitFor::None);
        Ok(pid)
    }

    /// Terminates process if it's alive.
    /// Doesn't attempt to switch to a new process.
    /// Used to terminate processes when e.g. their owner process dies.
    pub fn terminate(&mut self, target: ProcessId, status: ProcessResult) {
        if let Some(process) = self.processes.remove(&target) {
            log::info!("Stopping pid {} with status {:?}", target, status);

            if process.repeat_syscall {
                log::info!(" [system call was pending]");
            }

            // Do not schedule this process again, and wake up all
            // processes waiting for the termination of this one
            self.queues.on_process_over(process.id());

            // Close open ipc subscriptions and mailboxes
            {
                let mut ipc_manager = crate::ipc::IPC.try_lock().expect("IPC locked");
                ipc_manager.on_process_over(self, process.id(), status.clone());
            }

            // Publish the death of the process
            crate::ipc::kernel_publish(
                self,
                "process/terminated",
                &d7abi::ipc::protocol::ProcessTerminated {
                    pid: process.id(),
                    result: status,
                },
            );

            // TODO: Remove process data:
            // * Free stack frames, etc.
        }

        if self.running == Some(target) {
            self.running = None;
            self.running_timeslice_end = None;
        }
    }

    /// Terminates process if it's alive.
    /// Returns the data for the process to switch to, if any.
    /// Will never return `ProcessSwitch::Continue`.
    pub fn terminate_and_switch(
        &mut self, target: ProcessId, status: ProcessResult,
    ) -> ProcessSwitch {
        let is_current = self.running == Some(target);
        self.terminate(target, status);

        unsafe {
            if is_current {
                self.switch(None)
            } else {
                self.switch_current_or_next()
            }
        }
    }

    /// Store process information before switching to other process.
    /// Panics if the process doesn't exist.
    pub fn store_state(&mut self, pid: ProcessId, page_table: PhysAddr, stack_pointer: VirtAddr) {
        if let Some(p) = self.processes.get_mut(&pid) {
            // p.page_table = page_table;
            assert_eq!(p.page_table.p4_addr(), page_table, "???");
            p.stack_pointer = stack_pointer;
        } else {
            panic!("No such process pid {}", pid);
        }
    }

    /// Prepare switch to the next process
    /// Returns the data for the process to switch to, if any.
    /// If `schedule` is None, the current process will not be scheduled again.
    pub unsafe fn switch(&mut self, schedule: Option<WaitFor>) -> ProcessSwitch {
        if let Some(s) = schedule {
            if let Some(running_pid) = self.running {
                self.queues.give(running_pid, s);
            }
        }

        if let Some(pid) = self.queues.take() {
            self.running = Some(pid);
            self.running_timeslice_end =
                Some(BSPInstant::now().add_ticks(ns_to_ticks(TIME_SLICE_NS)));
            let process = self
                .processes
                .get_mut(&pid)
                .expect("Process from queue not running");
            if process.repeat_syscall {
                log::trace!("Repeat syscall");
                ProcessSwitch::RepeatSyscall(process.switch_info())
            } else {
                log::trace!("Switch to {}", pid);
                ProcessSwitch::Switch(process.switch_info())
            }
        } else {
            log::trace!("Switch to idle");
            self.running = None;
            self.running_timeslice_end = None;
            ProcessSwitch::Idle
        }
    }

    /// Prepare a "switch" to the current te process, if any.
    /// If no process is currently running, the next process queued to run
    /// is activated instead. If there is no active processes, simply idles.
    /// This is used when a concrete switch to current process is required.
    pub unsafe fn switch_current_or_next(&mut self) -> ProcessSwitch {
        if self.running.is_none() {
            self.running = self.queues.take();
        }

        if let Some(pid) = self.running {
            let process = self
                .processes
                .get_mut(&pid)
                .expect("self.running does not exist anymore");
            if process.repeat_syscall {
                ProcessSwitch::RepeatSyscall(process.switch_info())
            } else {
                ProcessSwitch::Switch(process.switch_info())
            }
        } else {
            ProcessSwitch::Idle
        }
    }

    /// Returns process to switch to, if any, and deadline for the next tick
    pub fn tick(&mut self) -> ProcessSwitch {
        let now = BSPInstant::now();
        self.queues.on_tick(&now);
        let target = unsafe { self.switch(Some(WaitFor::None)) };

        target
    }

    /// When `tick()` should be called again
    pub fn next_tick(&self) -> Option<BSPInstant> {
        let mut wakeup = self.queues.next_wakeup();

        if let Some(slice_end) = self.running_timeslice_end {
            wakeup = Some(wakeup.map(|w| w.min(slice_end)).unwrap_or(slice_end));
        };

        wakeup.map(|w| w.min(BSPInstant::now().add_ns(MIN_EXEC_TIME_NS)))
    }

    /// Tries to resolve a WaitFor in the current context
    pub fn try_resolve_waitfor(&self, waitfor: WaitFor) -> Result<ProcessId, WaitFor> {
        waitfor.try_resolve_immediate(&self.queues, self.running.expect("No process running"))
    }

    /// Relay events to queues
    pub fn on_explicit_event(&mut self, event_id: ExplicitEventId) {
        self.queues.on_explicit_event(event_id);
    }

    /// Full-screen view of the current scheduler status
    pub fn debug_view_string(&self) -> String {
        let mut lines = format!(
            "## SCHEDULER OVERVIEW ##  Currently running {:?}\n",
            self.running
        );
        lines.push_str(&self.queues.debug_view_string());
        lines
    }
}

lazy_static::lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = unsafe {
        Mutex::new(Scheduler::new())
    };
}

/// Exception handler doesn't schdule new slices if this isn't set
pub static SCHEDULER_ENABLED: AtomicBool = AtomicBool::new(false);
