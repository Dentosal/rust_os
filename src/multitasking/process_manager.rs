use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use spin::Mutex;

use alloc::vec::Vec;

use super::process::Process;
use super::ProcessId;

pub struct State {
    process_list: Vec<Process>,
    id_counter: ProcessId,
}

impl State {
    pub const unsafe fn new() -> Self {
        Self {
            process_list: Vec::new(),
            id_counter: ProcessId(0),
        }
    }

    /// Returns process by index for context switch
    pub fn get_at(&mut self, index: usize) -> Option<&mut Process> {
        self.process_list.get_mut(index)
    }

    /// Returns process count
    pub fn process_count(&self) -> usize {
        self.process_list.len()
    }

    /// Returns process ids
    pub fn process_ids(&self) -> Vec<ProcessId> {
        self.process_list.iter().map(|p| p.id()).collect()
    }

    /// Creates a new process without a parent process
    fn create_process(&mut self, parent: Option<ProcessId>) -> ProcessId {
        let pids = self.process_ids();
        rprintln!("L>");

        // Infinite loop is not possible, since we will never have 2**32 * 1000 bytes = 4.3 terabytes of memory for process list only
        while pids.contains(&self.id_counter) {
            self.id_counter = self.id_counter.next();
            rprintln!("L=");
        }
        rprintln!("L<");

        let process = Process::new(self.id_counter, parent);
        let pid = process.id();
        // TODO: populate process
        self.process_list.push(process);
        self.id_counter = self.id_counter.next();
        pid
    }

    /// Creates a new process without a parent process
    pub fn spawn(&mut self) -> ProcessId {
        unsafe {
            rforce_unlock!();
        }
        bochs_magic_bp!();
        rprintln!("Spawn;");
        self.create_process(None)
    }

    /// Forks existing process, and returns the id of the created child processes
    // pub fn fork(&mut self, target: ProcessId) -> ProcessId {
    //     self.create_process(Some(target))
    // }

    /// Kills process, and returns whether the process existed at all
    pub fn kill(&mut self, target: ProcessId, status_code: u64) -> bool {
        match self.process_list.iter().position(|p| p.id() == target) {
            Some(index) => {
                // TODO: Send return code to subscribed processes
                self.process_list.swap_remove(index);
                true
            }
            None => false,
        }
    }
}

/// Wrapper for State
pub struct ProcessManager(UnsafeCell<Mutex<State>>);
unsafe impl Sync for ProcessManager {}
impl ProcessManager {
    pub const unsafe fn new() -> Self {
        Self(UnsafeCell::new(Mutex::new(State::new())))
    }

    pub fn try_fetch<F, T>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&State) -> T,
    {
        if let Some(ref state) = unsafe { (*self.0.get()).try_lock() } {
            Some(f(state))
        } else {
            None
        }
    }

    pub fn try_update<F, T>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&mut State) -> T,
    {
        if let Some(ref mut state) = unsafe { (*self.0.get()).try_lock() } {
            Some(f(state))
        } else {
            None
        }
    }

    pub fn fetch<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&State) -> T,
    {
        self.try_fetch(f).expect("Unable to lock process manager")
    }

    pub fn update<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut State) -> T,
    {
        self.try_update(f).expect("Unable to lock process manager")
    }
}
