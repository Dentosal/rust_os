use d7time::{Duration, Instant};

use super::PROCMAN;
use super::process_manager::ProcessManager;


// const TIME_SLICE_MIRCOSECONDS: u64 = 1_000; // run task 1 millisecond and switch
const TIME_SLICE_MIRCOSECONDS: u64 = 1_000_000; // XXX: tesiting with 1 sec slices

pub struct Scheduler {
    next_switch: Option<Instant>,
    current_index: usize
}
impl Scheduler {
    pub const fn new() -> Scheduler{
        Scheduler {
            next_switch: None,
            current_index: 0
        }
    }

    // Returns process index to switch to, wrapped in option
    fn next(&mut self) -> Option<usize> {
        let pm = PROCMAN.lock();
        let pc = pm.process_count();
        if pc == 0 {
            None
        }
        else {
            self.current_index = (self.current_index+1) % pc;
            Some(self.current_index)
        }
    }

    // Switches to next process
    unsafe fn switch(&mut self) {
        let next = self.next();
        match next {
            Some(index) => {
                let mut pm = PROCMAN.lock();
                match pm.get_at(index) {
                    Some(ref mut process) => {
                        rforce_unlock!();
                        rprintln!("NP: {:}", process.id);
                    },
                    None => {} // A process was killed, invalidating the index
                    // TODO: ^^^^ is that even possible?
                }
            },
            None => {} // No processes
        }
    }

    pub fn tick(&mut self, now: Instant) {
        return;
        // unsafe {
        //     PROCMAN.force_unlock();
        // }

        match self.next_switch {
            Some(s) => {
                if now >= s {
                    self.next_switch = Some(now + Duration::from_millis(TIME_SLICE_MIRCOSECONDS));
                    unsafe {
                        self.switch();
                    }
                }
            },
            None => {
                // start switching
                self.next_switch = Some(now + Duration::from_millis(TIME_SLICE_MIRCOSECONDS));
            }
        }
    }
}
