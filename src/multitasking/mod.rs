use collections::Vec;
use spin::Mutex;

mod process;
mod process_manager;
mod scheduler;

use self::process_manager::ProcessManager;
use self::scheduler::Scheduler;

type ProcessId = u32;

pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
pub static PROCMAN: Mutex<ProcessManager> = Mutex::new(ProcessManager::new());

pub fn init() {
    PROCMAN.lock().init();
}
