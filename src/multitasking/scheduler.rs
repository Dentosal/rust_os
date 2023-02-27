use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use hashbrown::HashMap;
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};

use crate::memory::phys::OutOfMemory;
use crate::multitasking::ExplicitEventId;
use crate::smp::command::Command;
use crate::smp::sleep::ns_to_ticks;
use crate::smp::{current_processor_id, ProcessorId};
use crate::time::TscInstant;
use crate::{memory, smp};

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

#[derive(Debug, Clone, Copy)]
pub struct RunningProcess {
    pid: ProcessId,
    timeslice_end: TscInstant,
}

#[derive(Debug)]
pub struct Scheduler {
    /// Processes by id
    processes: HashMap<ProcessId, Process>,
    /// Queues for different types of scheduling
    queues: Queues,
    /// Id of the currently running process, for each CPU core
    running: HashMap<ProcessorId, RunningProcess>,
    /// Next available process id
    next_pid: ProcessId,
}
impl Scheduler {
    pub unsafe fn new() -> Self {
        Self {
            processes: HashMap::new(),
            queues: Queues::new(),
            running: HashMap::new(),
            next_pid: ProcessId::first(),
        }
    }

    /// Get id of the process running on a given cpu core
    pub fn get_running_pid(&self, processor: ProcessorId) -> Option<ProcessId> {
        self.running.get(&processor).map(|rp| rp.pid)
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
    /// Silently ignores nonexistent processes.
    pub fn terminate(&mut self, target: ProcessId, status: ProcessResult) {
        if let Some(process) = self.processes.remove(&target) {
            log::info!("Stopping pid {} with status {:?}", target, status);

            if process.repeat_syscall {
                log::info!(" [system call was pending]");
            }

            let running_on_cpu = self
                .running
                .iter()
                .find_map(|(cpu, rp)| if rp.pid == target { Some(*cpu) } else { None });
            if let Some(cpu) = running_on_cpu {
                log::debug!(" [currently running on {cpu:?}]");
            } else {
                log::debug!(" [not currently running]");
            }

            if let Some(cpu) = running_on_cpu {
                // If the process is running on a different core, send an IPI
                // to terminate it immediately.
                let current_cpu = current_processor_id();
                if cpu == current_cpu {
                    let old = self.running.remove(&current_cpu).unwrap();
                    assert!(old.pid == target);
                } else {
                    smp::command::send(cpu, Command::StopTerminatedProcess(target));
                }
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

    /// Gets the process that the current processor should continue running,
    /// switching to a process if the cpu was idle and any processses are available.
    pub fn continuation(&mut self, signal_wakeup: bool) -> ProcessSwitch {
        let cpu = current_processor_id();

        if !self.running.contains_key(&cpu) {
            if let Some(pid) = self.queues.take() {
                log::debug!("Continuing running of {pid:?}");
                let old = self.running.insert(current_processor_id(), RunningProcess {
                    pid,
                    timeslice_end: TscInstant::now().add_ticks(ns_to_ticks(TIME_SLICE_NS)),
                });
                assert!(old.is_none());
            }
        }

        // If we have queued processes and idle cpus, wake them up
        if signal_wakeup && self.queues.ready_count() > 0 {
            let mut idle_cpus = smp::iter_active_cpus().filter(|c| !self.running.contains_key(c));
            if let Some(idle_cpu) = idle_cpus.next() {
                assert_ne!(idle_cpu, current_processor_id());
                log::debug!("Waking up an idle cpu to match load");
                smp::command::send(idle_cpu, Command::ProcessAvailable);
            }
        }

        if let Some(rp) = self.running.get(&cpu) {
            let process = self
                .processes
                .get_mut(&rp.pid)
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

    /// Yield the current process, ending it's remaining timeslice immediately.
    /// If `schedule` is None, the current process will not be scheduled again.
    pub fn yield_current(&mut self, schedule: Option<WaitFor>) {
        if let Some(running) = self.running.remove(&current_processor_id()) {
            if let Some(s) = schedule {
                self.queues.give(running.pid, s);
            }
        }
    }

    pub fn switch_current_or_next(&mut self) -> ProcessSwitch {
        if self.running.contains_key(&current_processor_id()) {
            ProcessSwitch::Continue
        } else {
            self.continuation(true)
        }
    }

    /// Returns process to switch to, if any, and deadline for the next tick
    pub fn tick(&mut self) -> ProcessSwitch {
        let now = TscInstant::now();
        self.queues.on_tick(&now);

        if let Some(running) = self.running.get(&current_processor_id()) {
            if now >= running.timeslice_end {
                self.yield_current(Some(WaitFor::None));
            } else {
                return ProcessSwitch::Continue;
            }
        }

        self.continuation(true)
    }

    /// When, if ever, `tick()` should be called again on this cpu
    pub fn next_tick(&self) -> Option<TscInstant> {
        let mut wakeup = self.queues.next_wakeup();

        if let Some(rp) = self.running.get(&current_processor_id()) {
            wakeup = Some(wakeup.unwrap_or(rp.timeslice_end).min(rp.timeslice_end));
        }

        wakeup.map(|w| w.min(TscInstant::now().add_ns(MIN_EXEC_TIME_NS)))
    }

    /// Relay events to queues
    pub fn on_explicit_event(&mut self, event_id: ExplicitEventId) {
        self.queues.on_explicit_event(event_id);
    }

    /// Full-screen view of the current scheduler status
    pub fn debug_view_string(&self) -> String {
        let mut lines = format!(
            "## SCHEDULER OVERVIEW ##\nCurrently running {:?}\n",
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
