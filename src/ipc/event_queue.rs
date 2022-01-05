use alloc::collections::VecDeque;

use crate::multitasking::{ExplicitEventId, WaitFor, SCHEDULER};

/// Queue with wakeup event blocking with max size
#[derive(Debug)]
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

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns:
    /// * Ok(Some(event)) when push successful and event should be triggered
    /// * Ok(None) when push successful but no event
    /// * Err(()) when the buffer is full
    pub fn push(&mut self, item: T) -> Result<Option<ExplicitEventId>, ()> {
        if self.queue.len() >= self.limit {
            Err(())
        } else {
            self.queue.push_back(item);
            Ok(self.event.take())
        }
    }

    /// Nonblocking, returns None if the queue is empty
    pub fn pop(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    pub fn pop_or_event(&mut self) -> Result<T, ExplicitEventId> {
        self.queue.pop_front().ok_or_else(|| self.get_event())
    }

    pub fn take_event(&mut self) -> Option<ExplicitEventId> {
        self.event.take()
    }

    fn get_event(&mut self) -> ExplicitEventId {
        *self.event.get_or_insert_with(WaitFor::new_event_id)
    }

    pub fn wait_for(&mut self) -> WaitFor {
        if self.queue.is_empty() {
            WaitFor::Event(self.get_event())
        } else {
            WaitFor::None
        }
    }

    pub fn into_iter(self) -> impl Iterator<Item = T> {
        self.queue.into_iter()
    }
}
