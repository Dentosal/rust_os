use collections::Vec;
use spin::Mutex;

use time::SystemClock;

mod process;
mod process_manager;

use self::process_manager::ProcessManager;

type ProcessId = u32;

const TIME_SLICE_MIRCOSECONDS: u64 = 1_000; // run task 1 millisecond and switch

struct Scheduler {
    process_manager: Option<ProcessManager>,
    next_switch: Option<SystemClock>,
    current_index: usize
}
impl Scheduler {
    pub const fn new() -> Scheduler{
        Scheduler {
            process_manager: None,
            next_switch: None,
            current_index: 0
        }
    }

    pub fn init(&mut self) {
        self.process_manager = Some(ProcessManager::new());
    }


    // Returns process index to switch to, wrapped in option
    fn next(&mut self) -> Option<usize> {
        let pc = self.process_manager.process_count();
        if pc == 0 {
            None
        }
        else {
            self.current_index = (self.current_index+1)%pc;
            Some(self.current_index)
        }
    }

    // Switches to next process
    unsafe fn switch(&mut self) {
        match self.next(self.process_manager.clone().unwrap()) {
            Some(index) => {
                let mut process = self.process_manager.get_at(index);
            },
            None => {}
        }

    }

    pub fn tick(&mut self, sysclock: SystemClock) {
        if self.process_manager.is_none() {
            // not initialized
            return;
        }

        match self.next_switch {
            Some(s) => {
                if s >= sysclock {
                    self.next_switch = Some(sysclock.after_microseconds(TIME_SLICE_MIRCOSECONDS));
                    unsafe {
                        self.switch();
                    }
                }
            },
            None => {
                // start switching
                self.next_switch = Some(sysclock.after_microseconds(TIME_SLICE_MIRCOSECONDS));
            }
        }
    }
}


pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

pub fn init() {
    SCHEDULER.lock().init();
}
