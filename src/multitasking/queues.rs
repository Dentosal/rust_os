use alloc::collections::VecDeque;
use alloc::prelude::v1::*;
use hashbrown::HashMap;

use d7abi::fs::FileDescriptor;
use d7time::Instant;

use crate::multitasking::ProcessId;

use super::WaitFor;

/// Internal wait id for scheduler queues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
struct WaitId(u64);
impl WaitId {
    /// Next file descriptor
    fn take(&mut self) -> Self {
        let old = *self;
        self.0 += 1;
        old
    }
}

#[derive(Debug)]
pub struct Queues {
    /// Processes currently in the running queue
    running: VecDeque<ProcessId>,
    /// Processes waiting for some trigger. Target for items in wait_*` queues.
    ///
    /// When a trigger has been reached once, the WaitId is consumed,
    /// and further times when the same WaitId is triggered are ignored.
    /// This allows multiple triggers for a process to be inserted,
    /// as only the first one actually triggers an event.
    /// This ensures that a process will never be returned twice to the schduler.
    waiting: HashMap<WaitId, ProcessId>,
    /// Next available WaitId
    next_waitid: WaitId,
    /// Processes which are sleeping until specified time
    /// Must be kept sorted by the wake-up time
    /// TODO: Switch to a proper priority queue for faster insert time
    wait_sleeping: VecDeque<(Instant, WaitId)>,
    /// Waiting for a process to complete.
    wait_process: HashMap<ProcessId, Vec<WaitId>>,
}
impl Queues {
    pub fn new() -> Self {
        Self {
            running: VecDeque::new(),
            waiting: HashMap::new(),
            next_waitid: WaitId(0),
            wait_sleeping: VecDeque::new(),
            wait_process: HashMap::new(),
        }
    }

    /// Is there a process with this in any queue
    pub fn process_exists(&self, pid: ProcessId) -> bool {
        self.running.contains(&pid) || self.waiting.values().any(|p| p == &pid)
    }

    fn create_wait(&mut self, pid: ProcessId) -> WaitId {
        let wait_id = self.next_waitid.take();
        self.waiting.insert(wait_id, pid);
        wait_id
    }

    /// If wait_id has benn consumed, ignores it.
    /// Otherwise the wait_id is consumed, and
    /// the associated process is scheduled for running.
    fn trigger_wait(&mut self, wait_id: WaitId) {
        if let Some(pid) = self.waiting.remove(&wait_id) {
            // TODO: can this cause starvation?
            self.running.push_front(pid);
        }
    }

    fn give_inner(&mut self, s: WaitFor, wait_id: WaitId) {
        match s {
            WaitFor::Time(instant) => {
                let i = p_index_vecdeque(&self.wait_sleeping, &instant);
                self.wait_sleeping.insert(i, (instant, wait_id));
            },
            WaitFor::Process(wait_for_pid) => {
                self.wait_process
                    .entry(wait_for_pid)
                    .or_default()
                    .push(wait_id);
            },
            WaitFor::None => {
                panic!("WaitFor::None inside of WaitFor::FirstOf");
            },
            WaitFor::FirstOf(targets) => {
                // Possible to support (simply recurse), but these
                // imply ineffiency or more serious issues elsewhere
                panic!("Nested WaitFor::FirstOf");
            },
        }
    }

    pub fn give(&mut self, pid: ProcessId, mut s: WaitFor) {
        s = s.reduce(&self, pid);
        if let WaitFor::None = s {
            self.running.push_back(pid);
            return;
        }

        let wait_id = self.create_wait(pid);
        if let WaitFor::FirstOf(targets) = s {
            for target in targets {
                self.give_inner(target, wait_id);
            }
        } else {
            self.give_inner(s, wait_id);
        }
    }

    /// Returns the process to run next, if any.
    /// The process is removed from all queues,
    /// and will not be returned again unless
    /// added using one of the give calls.
    pub fn take(&mut self) -> Option<ProcessId> {
        self.running.pop_front()
    }

    /// Update when clock ticks
    pub fn on_tick(&mut self, now: &Instant) {
        while let Some((wakeup, _)) = self.wait_sleeping.front() {
            if now >= wakeup {
                let (_, wait_id) = self.wait_sleeping.pop_front().unwrap();
                self.trigger_wait(wait_id);
            } else {
                break;
            }
        }
    }

    /// Update when a process completes
    pub fn on_process_over(&mut self, completed: ProcessId) {
        if let Some(wait_ids) = self.wait_process.remove(&completed) {
            for wait_id in wait_ids {
                self.trigger_wait(wait_id);
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
