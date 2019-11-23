use alloc::collections::VecDeque;
use alloc::prelude::v1::*;
use hashbrown::HashMap;

use d7time::Instant;

use crate::multitasking::ProcessId;

/// Instructions for scheduling a process
#[derive(Debug, Clone)]
pub enum WaitFor {
    /// Run again on the next free slot
    None,
    /// Run after specified moment
    Time(Instant),
    /// Process completed
    Process(ProcessId),
}

#[derive(Debug)]
pub struct Queues {
    /// Processes currently in the running queue
    pub running: VecDeque<ProcessId>,
    /// Processes which are sleeping until specified time
    /// Must be kept sorted by the wake-up time
    /// TODO: Switch to a proper priority queue for faster insert time
    pub sleeping: VecDeque<(Instant, ProcessId)>,
    /// Waiting for a process to complete.
    pub process: HashMap<ProcessId, Vec<ProcessId>>,
}
impl Queues {
    pub fn new() -> Self {
        Self {
            running: VecDeque::new(),
            sleeping: VecDeque::new(),
            process: HashMap::new(),
        }
    }

    pub fn all_completed(&self) -> bool {
        self.running.is_empty() && self.sleeping.is_empty()
    }

    pub fn give(&mut self, pid: ProcessId, s: WaitFor) {
        match s {
            WaitFor::None => {
                self.running.push_back(pid);
            },
            WaitFor::Time(instant) => {
                let i = p_index_vecdeque(&self.sleeping, &instant);
                self.sleeping.insert(i, (instant, pid));
            },
            WaitFor::Process(wait_for_pid) => {
                self.process.entry(wait_for_pid).or_default().push(pid);
            },
        }
    }

    pub fn take(&mut self) -> Option<ProcessId> {
        self.running.pop_front()
    }

    /// Update when clock ticks
    pub fn on_tick(&mut self, now: &Instant) {
        while let Some((wakeup, _)) = self.sleeping.front() {
            if now >= wakeup {
                let (_, pid) = self.sleeping.pop_front().unwrap();
                // FIXME: push_front to schedule immediately?
                self.running.push_back(pid);
            } else {
                break;
            }
        }
    }

    /// Update when a process completes
    pub fn on_process_over(&mut self, completed: ProcessId) {
        if let Some(pids) = self.process.remove(&completed) {
            for pid in pids {
                // FIXME: push_front to schedule immediately?
                self.running.push_back(pid);
            }
        }
    }
}

/// Priority queue like index in the vecdeque of pairs
/// The first item of the pair is used as the priority key
fn p_index_vecdeque<K: Ord, V>(v: &VecDeque<(K, V)>, t: &K) -> usize {
    // TODO: use binary search?
    let mut i = 0;
    while i < v.len() {
        if v[i].0 > *t {
            return i;
        }
        i += 1;
    }
    v.len()
}
