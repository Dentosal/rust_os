use alloc::collections::VecDeque;
use alloc::prelude::v1::*;

use d7time::Instant;

use crate::multitasking::ProcessId;

/// Instructions for scheduling a process
#[derive(Debug, Clone)]
pub enum Schedule {
    /// Run again on the next free slot
    Running,
    /// Run after specified moment
    Sleeping(Instant),
}

#[derive(Debug)]
pub struct Queues {
    /// Processes currently in the running queue
    pub running: VecDeque<ProcessId>,
    /// Processes which are sleeping until specified time
    /// Must be kept sorted by the wake-up time
    /// TODO: Switch to a proper priority queue for faster insert time
    pub sleeping: VecDeque<(Instant, ProcessId)>,
}
impl Queues {
    pub fn new() -> Self {
        Self {
            running: VecDeque::new(),
            sleeping: VecDeque::new(),
        }
    }

    pub fn all_completed(&self) -> bool {
        self.running.is_empty() && self.sleeping.is_empty()
    }

    pub fn give(&mut self, pid: ProcessId, s: Schedule) {
        match s {
            Schedule::Running => {
                self.running.push_back(pid);
            },
            Schedule::Sleeping(instant) => {
                let i = p_index_vecdeque(&self.sleeping, &instant);
                self.sleeping.insert(i, (instant, pid));
            },
        }
    }

    pub fn take(&mut self) -> Option<ProcessId> {
        self.running.pop_front()
    }

    /// Update by when clock ticks
    pub fn tick(&mut self, now: &Instant) {
        if self.sleeping.len() >= 2 {
            assert!(self.sleeping[0].0 < self.sleeping[1].0, "SLORD1");
            if self.sleeping.len() >= 3 {
                assert!(self.sleeping[1].0 < self.sleeping[2].0, "SLORD2");
            }
        }

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
