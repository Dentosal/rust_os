use alloc::prelude::v1::*;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use hashbrown::HashMap;
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};

use d7time::{Duration, Instant};

use crate::memory;
use crate::multitasking::loader::ElfImage;

use super::process::{self, Process, ProcessMetadata, ProcessResult};
use super::queues::{Queues, Schedule};
use super::ProcessId;

const TIME_SLICE: Duration = Duration::from_millis(1); // run task 1 millisecond and switch
// const TIME_SLICE: Duration = Duration::from_millis(1_000); // XXX: testing with 1 sec slices

/// Process switch an related alternatives
#[derive(Debug)]
pub enum ProcessSwitch {
    /// Switch to a different process
    Switch(Process),
    /// Keep the same process
    Continue,
    /// Go to idle state
    Idle,
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
    unsafe fn create_process(&mut self, parent: Option<ProcessId>, elf: ElfImage) -> ProcessId {
        let pid = self.next_pid;
        self.next_pid = self.next_pid.next();
        let process = memory::configure(|mm| Process::create(mm, pid, parent, elf));
        self.processes.insert(pid, process);
        self.queues.give(pid, Schedule::Running);
        pid
    }

    /// Creates a new process without a parent process
    pub fn spawn(&mut self, elf_image: ElfImage) -> ProcessId {
        unsafe { self.create_process(None, elf_image) }
    }

    /// Terminates process if it's alive.
    /// Returns whether the process existed at all.
    /// Note that `self.running` must be updated manually if changed
    fn terminate(&mut self, target: ProcessId, status: ProcessResult) -> bool {
        if let Some(process) = self.processes.remove(&target) {
            rprintln!("Stopping pid {} with status {:?}", target, status);
            // TODO: Send return code to subscribed processes
            // TODO: remove process data: free stack frames, etc.
            true
        } else {
            false
        }
    }

    /// Terminate the current process, and switch to the next one immediately.
    /// Returns the data for the process to switch to.
    /// If there are no processes left, panics as there is nothing to do.
    pub unsafe fn terminate_current(&mut self, status: process::ProcessResult) -> ProcessSwitch {
        let pid = self.get_running_pid().expect("No process running");
        self.running = None;
        self.terminate(pid, status);
        self.switch(None)
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
    /// If `schedule` is None, the process will not be scheduled again.
    pub unsafe fn switch(&mut self, schedule: Option<Schedule>) -> ProcessSwitch {
        if let Some(pid) = self.queues.take() {
            if let Some(s) = schedule {
                if let Some(running_pid) = self.running {
                    self.queues.give(running_pid, s);
                }
            }
            self.running = Some(pid);
            ProcessSwitch::Switch(self.processes.get(&pid).unwrap().clone())
        } else {
            if let Some(s) = schedule {
                if let Some(running_pid) = self.running {
                    self.queues.give(running_pid, s);
                }
            }
            self.running = None;
            ProcessSwitch::Idle
        }
    }

    pub fn tick(&mut self, now: Instant) -> ProcessSwitch {
        self.queues.tick(&now);
        match self.next_switch {
            Some(s) => {
                if now >= s {
                    self.next_switch = Some(now + TIME_SLICE);
                    unsafe { self.switch(Some(Schedule::Running)) }
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
