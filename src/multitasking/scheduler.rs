use alloc::prelude::v1::*;
use hashbrown::HashMap;
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};

use d7time::{Duration, Instant};

use crate::memory;
use crate::multitasking::loader::ElfImage;

use super::process::{Process, ProcessResult};
use super::queues::{Queues, WaitFor};
use super::ProcessId;

const TIME_SLICE: Duration = Duration::from_millis(1); // run task 1 millisecond and switch

/// Process switch an related alternatives
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum ProcessSwitch {
    /// Keep the same process
    Continue,
    /// Go to idle state
    Idle,
    /// Switch to a different process
    Switch(Process),
    /// Switch to a new process, by repeating
    /// a system call, and continuing after that.
    /// Syscall number and arguments must be stored in the process
    RepeatSyscall(Process),
}

pub struct Scheduler {
    /// Time of the next switch if set, otherwise immediately
    next_switch: Option<Instant>,
    /// Processes by id
    processes: HashMap<ProcessId, Process>,
    /// Queues for different types of scheduling
    queues: Queues,
    /// Id of the currently running process
    running: Option<ProcessId>,
    /// Next available process id
    next_pid: ProcessId,
}
impl Scheduler {
    pub unsafe fn new() -> Self {
        Self {
            next_switch: None,
            processes: HashMap::new(),
            queues: Queues::new(),
            running: None,
            next_pid: ProcessId::first(),
        }
    }

    /// Get id of the current process
    pub fn get_running_pid(&self) -> Option<ProcessId> {
        self.running
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
    unsafe fn create_process(&mut self, elf: ElfImage) -> ProcessId {
        let pid = self.next_pid;
        self.next_pid = self.next_pid.next();
        let process = memory::configure(|mm| Process::create(mm, pid, elf));
        self.processes.insert(pid, process);
        self.queues.give(pid, WaitFor::None);
        pid
    }

    /// Creates a new process without a parent process
    pub fn spawn(&mut self, elf_image: ElfImage) -> ProcessId {
        unsafe { self.create_process(elf_image) }
    }

    /// Terminates process if it's alive.
    /// Returns the data for the process to switch to, if any.
    /// Will never return `ProcessSwitch::Continue`.
    pub fn terminate_and_switch(
        &mut self, target: ProcessId, status: ProcessResult,
    ) -> ProcessSwitch {
        use crate::filesystem::FILESYSTEM;

        if let Some(process) = self.processes.remove(&target) {
            rprintln!("Stopping pid {} with status {:?}", target, status);
            // Schedule processes waiting for the termination
            self.queues.on_process_over(process.id());
            // Close open file pointers
            FILESYSTEM
                .try_lock()
                .expect("FILESYSTEM LOCKED")
                .on_process_over(process.id());

            // TODO: Remove process data:
            // * Free stack frames, etc.

            if self.running == Some(process.id()) {
                self.running = None;
                unsafe { self.switch(None) }
            } else {
                unsafe { self.switch_current() }
            }
        } else {
            ProcessSwitch::Idle
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
            let process = self.processes.get_mut(&pid).unwrap();
            if process.repeat_syscall {
                ProcessSwitch::RepeatSyscall(process.clone())
            } else {
                ProcessSwitch::Switch(process.clone())
            }
        } else {
            self.running = None;
            ProcessSwitch::Idle
        }
    }

    /// Prepare switch to the current process if any.
    /// This is used when a concrete switch to current process is required.
    /// If there is no active process, simply idles.
    pub unsafe fn switch_current(&mut self) -> ProcessSwitch {
        if let Some(pid) = self.running {
            let process = self.processes.get_mut(&pid).unwrap();
            if process.repeat_syscall {
                ProcessSwitch::RepeatSyscall(process.clone())
            } else {
                ProcessSwitch::Switch(process.clone())
            }
        } else {
            ProcessSwitch::Idle
        }
    }

    pub fn tick(&mut self, now: Instant) -> ProcessSwitch {
        self.queues.on_tick(&now);
        match self.next_switch {
            Some(s) => {
                if now >= s {
                    self.next_switch = Some(now + TIME_SLICE);
                    unsafe { self.switch(Some(WaitFor::None)) }
                } else {
                    ProcessSwitch::Continue
                }
            },
            None => {
                // start switching
                self.next_switch = Some(now + TIME_SLICE);
                ProcessSwitch::Continue
            },
        }
    }
}

lazy_static::lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = unsafe {
        Mutex::new(Scheduler::new())
    };
}
