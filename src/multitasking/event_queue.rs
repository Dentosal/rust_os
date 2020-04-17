use alloc::collections::VecDeque;

use crate::filesystem::result::IoResult;
use crate::multitasking::{ExplicitEventId, WaitFor, SCHEDULER};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueLimit {
    /// No limit
    None,
    /// Drops events after limit
    Soft(usize),
    /// Panics after limit
    Hard(usize),
}

/// Queue with wakeup event blocking with max size
#[derive(Debug)]
pub struct EventQueue<T> {
    // Name used to identify the queue in logs
    name: &'static str,
    queue: VecDeque<T>,
    event: Option<ExplicitEventId>,
    limit: QueueLimit,
}
impl<T> EventQueue<T> {
    pub fn new(name: &'static str, limit: QueueLimit) -> Self {
        Self {
            name,
            queue: VecDeque::new(),
            event: None,
            limit,
        }
    }

    /// Push in non-IO context, when scheduler is not locked
    pub fn push(&mut self, item: T) {
        if let Some(event_id) = self._push(item) {
            let mut sched = SCHEDULER.try_lock().unwrap();
            sched.on_explicit_event(event_id);
        }
    }

    /// Push in IO context, i.e. returns event if it occurs
    pub fn push_io(&mut self, item: T) -> IoResult<()> {
        if let Some(event) = self._push(item) {
            IoResult::success(()).with_event(event)
        } else {
            IoResult::success(())
        }
    }

    #[inline]
    pub fn _push(&mut self, item: T) -> Option<ExplicitEventId> {
        self.queue.push_back(item);

        let l = self.queue.len();
        match self.limit {
            QueueLimit::Soft(limit) if l > limit => {
                self.queue.remove(0);
                log::warn!("{}: Buffer full, discarding event", self.name);
            },
            QueueLimit::Hard(limit) if l > limit => {
                panic!("{}: Buffer full (hard limit)", self.name);
            },
            _ => {},
        }

        self.event.take()
    }

    /// Nonblocking, returns None if the queue is empty
    pub fn pop_event(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    /// Soft blocking, to be used in IO contexts
    pub fn io_pop_event(&mut self) -> IoResult<T> {
        if let Some(event) = self.pop_event() {
            IoResult::success(event)
        } else {
            IoResult::repeat_after(WaitFor::Event(self.get_event()))
        }
    }

    pub fn get_event(&mut self) -> ExplicitEventId {
        let name = self.name.clone();
        *self.event.get_or_insert_with(WaitFor::new_event_id)
    }

    pub fn wait_for(&mut self) -> WaitFor {
        if self.queue.is_empty() {
            WaitFor::Event(self.get_event())
        } else {
            WaitFor::None
        }
    }
}
