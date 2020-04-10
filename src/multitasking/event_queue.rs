use alloc::collections::VecDeque;

use crate::filesystem::error::IoResult;
use crate::multitasking::{ExplicitEventId, WaitFor, SCHEDULER};

/// Queue with wakeup event blocking with max size
/// Silently drops events after maxsize has been reached
pub struct EventQueue<T> {
    queue: VecDeque<T>,
    event: Option<ExplicitEventId>,
    limit: usize,
}
impl<T> EventQueue<T> {
    pub fn new(limit: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            event: None,
            limit,
        }
    }

    pub fn push(&mut self, item: T) {
        self.queue.push_back(item);

        if let Some(event_id) = self.event.take() {
            let mut sched = SCHEDULER.try_lock().unwrap();
            sched.on_explicit_event(event_id);
        }

        if self.queue.len() > self.limit {
            self.queue.remove(0);
            // TODO: log full buffer
        }
    }

    /// Nonblocking, returns None if the queue is empty
    pub fn pop_event(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    /// Nonblocking, to be used in IO contexts
    pub fn io_pop_event(&mut self) -> IoResult<T> {
        if let Some(event) = self.pop_event() {
            IoResult::Success(event)
        } else {
            IoResult::RepeatAfter(WaitFor::Event(self.get_event()))
        }
    }

    pub fn get_event(&mut self) -> ExplicitEventId {
        *self.event.get_or_insert_with(WaitFor::new_event_id)
    }
}
