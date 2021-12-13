use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use hashbrown::HashSet;

use crate::multitasking::ProcessId;
use crate::time::BSPInstant;

use super::queues::Queues;

/// Instructions for scheduling a process
#[derive(Debug, Clone, PartialEq)]
pub enum WaitFor {
    /// Run again on the next free slot
    None,
    /// Run after specified moment
    Time(BSPInstant),
    /// Process completed
    Process(ProcessId),
    /// An explicitly-triggered event
    Event(ExplicitEventId),
    /// First of multiple wait conditions.
    /// Should never contain `None`.
    FirstOf(Vec<WaitFor>),
}
impl WaitFor {
    /// Resolve the condition immediately, if possible
    pub fn try_resolve_immediate(self, qs: &Queues, current: ProcessId) -> Result<ProcessId, Self> {
        use WaitFor::*;

        let process_done = |p| current != p && !qs.process_exists(p);
        match &self {
            Process(p) if process_done(*p) => {
                return Ok(*p);
            },
            FirstOf(subevents) => {
                for e in subevents.iter() {
                    if let Process(p) = e {
                        if process_done(*p) {
                            return Ok(*p);
                        }
                    }
                }
            },
            _ => {},
        }

        Err(self)
    }

    /// Minimize based on current conditions.
    /// Used by scheduler queues to make sure that
    /// completed processes are not waited for.
    pub fn reduce_queues(self, qs: &Queues, current: ProcessId) -> Self {
        use WaitFor::*;

        let process_done = |p| current != p && !qs.process_exists(p);
        match &self {
            Process(p) if process_done(*p) => {
                return None;
            },
            FirstOf(subevents) => {
                for e in subevents.iter() {
                    if let Process(p) = e {
                        if process_done(*p) {
                            return None;
                        }
                    }
                }
            },
            _ => {},
        }

        self.reduce()
    }
    /// Minimize based on conditions.
    /// Used to make sure that process that is already
    /// completed is not waited for.
    pub fn reduce(self) -> Self {
        use WaitFor::*;
        match self {
            FirstOf(mut subevents) => {
                let mut new_se = Vec::new();
                let mut earliest: Option<BSPInstant> = Option::None;
                for e in subevents.into_iter() {
                    match e {
                        None => {
                            panic!("None in FirstOf");
                        },
                        Time(instant) => {
                            if let Some(e) = earliest {
                                if instant < e {
                                    earliest = Some(instant);
                                }
                            } else {
                                earliest = Some(instant);
                            }
                        },
                        FirstOf(_) => {
                            panic!("NESTED FirstOf in reduce");
                        },
                        other => {
                            new_se.push(other);
                        },
                    }
                }

                if let Some(e) = earliest {
                    new_se.push(Time(e));
                }

                if new_se.is_empty() {
                    None
                } else if new_se.len() == 1 {
                    new_se.pop().unwrap()
                } else {
                    FirstOf(new_se)
                }
            },
            other => other,
        }
    }

    pub fn new_event_id() -> ExplicitEventId {
        ExplicitEventId(NEXT_EVENT.fetch_add(1, Ordering::SeqCst))
    }
}

/// Manually triggerable event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExplicitEventId(u64);

static NEXT_EVENT: AtomicU64 = AtomicU64::new(0);
