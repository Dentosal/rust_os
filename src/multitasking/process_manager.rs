use super::ProcessId;
use super::process::Process;

use collections::Vec;

pub struct ProcessManager {
    processes: Vec<Process>,
    id_counter: ProcessId
}

impl ProcessManager {
    pub fn new() -> ProcessManager {
        ProcessManager {
            processes: Vec::new(),
            id_counter: 0
        }
    }

    /// Returns process by index for context switch
    pub fn get_at(&mut self, index: usize) -> Option<&mut Process> {
        self.processes.get_mut(index)
    }

    /// Returns process count
    pub fn process_count(&self) -> usize {
        self.processes.len()
    }

    /// Returns process count
    pub fn process_ids(&self) -> Vec<ProcessId> {
        self.processes.iter().map(|p| p.id).collect()
    }

    /// Creates a new process without a parent process
    fn create_process(&mut self, parent: Option<ProcessId>) -> ProcessId {
        let pids = self.process_ids();

        // Infinite loop is not possible, since we will never have 2**32 * 1000 bytes = 4.3 terabytes of memory for process list only
        while pids.contains(&self.id_counter) {
            self.id_counter = self.id_counter.checked_add(1).unwrap_or(0);
        }

        let process = Process::new(self.id_counter, parent);
        let pid = process.id;
        // TODO: populate process
        self.processes.push(process);
        self.id_counter = self.id_counter.checked_add(1).unwrap_or(0);
        pid
    }

    /// Creates a new process without a parent process
    pub fn spawn(&mut self) -> ProcessId {
        self.create_process(None)
    }

    /// Forks existing process, and returns the id of the created child processes
    pub fn fork(&mut self, target: ProcessId) -> ProcessId {
        self.create_process(Some(target))
    }

    /// Kills process, and returns whether the process existed at all
    /// TODO: signal?
    pub fn kill(&mut self, target: ProcessId) -> bool {
        match self.processes.iter().position(|p| p.id == target) {
            Some(index) => {
                self.processes.remove(index);
                true
            },
            None => false
        }
    }
}
