use super::ProcessId;
use super::process::Process;

use collections::Vec;

pub struct ProcessManager {
    process_list: Option<Vec<Process>>,
    id_counter: ProcessId
}

macro_rules! unwrap_ref {
    ($e:expr) => (
        match $e {
            Some(ref x) => x,
            None => panic!("Cannot unwrap None!")
        }
    );
}
macro_rules! unwrap_ref_mut {
    ($e:expr) => (
        match $e {
            Some(ref mut x) => x,
            None => panic!("Cannot unwrap None!")
        }
    );
}

impl ProcessManager {
    pub const fn new() -> ProcessManager {
        ProcessManager {
            process_list: None,
            id_counter: 0
        }
    }

    pub fn init(&mut self) {
        self.process_list = Some(Vec::new());
    }

    pub fn is_initialized(&mut self) -> bool {
        self.process_list.is_some()
    }

    /// Returns process by index for context switch
    pub fn get_at(&mut self, index: usize) -> Option<&mut Process> {
        unwrap_ref_mut!(self.process_list).get_mut(index)
    }

    /// Returns process count
    pub fn process_count(&self) -> usize {
        unwrap_ref!(self.process_list).len()
    }

    /// Returns process count
    pub fn process_ids(&self) -> Vec<ProcessId> {
        unwrap_ref!(self.process_list).iter().map(|p| p.id).collect()
    }

    /// Creates a new process without a parent process
    fn create_process(&mut self, parent: Option<ProcessId>) -> ProcessId {
        let pids = self.process_ids();
        rprintln!("L>");

        // Infinite loop is not possible, since we will never have 2**32 * 1000 bytes = 4.3 terabytes of memory for process list only
        while pids.contains(&self.id_counter) {
            self.id_counter = self.id_counter.checked_add(1).unwrap_or(0);
            rprintln!("L=");
        }
        rprintln!("L<");

        let process = Process::new(self.id_counter, parent);
        let pid = process.id;
        // TODO: populate process
        unwrap_ref_mut!(self.process_list).push(process);
        self.id_counter = self.id_counter.checked_add(1).unwrap_or(0);
        pid
    }

    /// Creates a new process without a parent process
    pub fn spawn(&mut self) -> ProcessId {
        unsafe{rforce_unlock!();}
        bochs_magic_bp!();
        rprintln!("Spawn;");
        self.create_process(None)
    }

    /// Forks existing process, and returns the id of the created child processes
    pub fn fork(&mut self, target: ProcessId) -> ProcessId {
        self.create_process(Some(target))
    }

    /// Kills process, and returns whether the process existed at all
    /// TODO: signal?
    pub fn kill(&mut self, target: ProcessId) -> bool {
        match unwrap_ref!(self.process_list).iter().position(|p| p.id == target) {
            Some(index) => {
                unwrap_ref_mut!(self.process_list).remove(index);
                true
            },
            None => false
        }
    }
}
