use core::cell::UnsafeCell;
use spin::Mutex;

use d7time::{Duration, Instant};

use super::process::{self, Process, ProcessMetadata};
use super::ProcessId;
use super::PROCMAN;

// const TIME_SLICE: Duration = Duration::from_millis(1); // run task 1 millisecond and switch
const TIME_SLICE: Duration = Duration::from_millis(1_000); // XXX: testing with 1 sec slices

struct State {
    /// Time of the next switch if set, otherwise immediately
    next_switch: Option<Instant>,
    /// Index of the current item
    current_index: usize,
    /// Current process metadata, cached for faster access and
    /// to protect against changes in process manager state. which can:
    /// * make the index point to another process
    /// * remove (kill) the process altogether
    current_process_metadata: Option<ProcessMetadata>,
}
impl State {
    pub const unsafe fn new() -> Self {
        Self {
            next_switch: None,
            current_index: 0,
            current_process_metadata: None,
        }
    }

    /// Get id of the current process
    pub fn get_running_pid(&self) -> Option<ProcessId> {
        self.current_process_metadata.clone().map(|p_md| p_md.id)
    }

    /// Terminate the current process, and switch to the next one immediately.
    /// Returns the data for the process to switch to.
    /// If there are no processes left, panics as there is nothing to do.
    unsafe fn terminate_current(&mut self, status: process::Status) -> Process {
        let pid = self.get_running_pid().expect("No process running?");
        self.next_switch = None;
        self.current_index = 0;
        self.current_process_metadata = None;
        PROCMAN.update(|pm| {
            pm.terminate(pid, status);
            if let Some(ref mut process) = pm.get_at(0) {
                self.current_process_metadata = Some(process.metadata());
                rprintln!("T -> {:?}", process);
                process.clone()
            } else {
                // No processes running. Halt.
                panic!("All processes over");
            }
        })
    }

    /// Prepare switch to the next process
    /// Returns the data for the process to switch to, if any
    unsafe fn prepare_switch(&mut self) -> Option<Process> {
        PROCMAN
            .try_update(|pm| {
                let pc = pm.process_count();

                if pc == 0 {
                    return None;
                }

                // Will not overflow, since there will never be usize::MAX processes running
                self.current_index = (self.current_index + 1) % pc;
                let mut process = pm.get_at(self.current_index)?;
                self.current_process_metadata = Some(process.metadata());
                rprintln!("S -> {:?}", process);
                Some(process.clone())
            })
            .expect("PreSwitch: Couldn't lock process manager")
    }

    pub fn tick(&mut self, now: Instant) -> Option<Process> {
        match self.next_switch {
            Some(s) => {
                if now >= s {
                    self.next_switch = Some(now + TIME_SLICE);
                    unsafe { self.prepare_switch() }
                } else {
                    None
                }
            },
            None => {
                // start switching
                self.next_switch = Some(now + TIME_SLICE);
                None
            },
        }
    }
}

/// Wrapper for State
pub struct Scheduler(UnsafeCell<Mutex<State>>);
unsafe impl Sync for Scheduler {}
impl Scheduler {
    pub const unsafe fn new() -> Self {
        Self(UnsafeCell::new(Mutex::new(State::new())))
    }

    pub unsafe fn tick(&self, now: Instant) -> Option<Process> {
        if let Some(ref mut state) = (*self.0.get()).try_lock() {
            state.tick(now)
        } else {
            // TODO: Just skip this, or maybe just log this?
            panic!("Unable to lock scheduler");
        }
    }

    pub fn get_running_pid(&self) -> Option<ProcessId> {
        unsafe {
            if let Some(ref state) = (*self.0.get()).try_lock() {
                state.get_running_pid()
            } else {
                // TODO: Just return? Result<Option<ProcessId>, ()>
                panic!("Unable to lock scheduler");
            }
        }
    }

    pub fn terminate_current(&self, status: process::Status) -> Process {
        unsafe {
            if let Some(ref mut state) = (*self.0.get()).try_lock() {
                state.terminate_current(status)
            } else {
                // TODO: What to do here?
                panic!("Unable to lock scheduler");
            }
        }
    }

    /// Forcibly yield control to next process
    /// Blocks if not available
    pub unsafe fn force_yield(&self) -> ! {
        loop {
            if let Some(ref mut state) = (*self.0.get()).try_lock() {
                unimplemented!();
            }
        }
    }
}
