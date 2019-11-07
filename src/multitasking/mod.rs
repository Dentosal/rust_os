use core::fmt;
use spin::Mutex;

mod loader;
pub mod process;
mod process_manager;
mod scheduler;

pub use self::loader::{load_module, ElfImage};
pub use self::process::Process;
use self::process_manager::ProcessManager;
use self::scheduler::Scheduler;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(u64);
impl ProcessId {
    fn next(&self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}
impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub static SCHEDULER: Scheduler = unsafe { Scheduler::new() };
pub static PROCMAN: ProcessManager = unsafe { ProcessManager::new() };

/// Forcibly yield control to next process
/// Blocks if not available
#[naked]
pub unsafe fn on_process_over() -> ! {
    SCHEDULER.force_yield();
}
