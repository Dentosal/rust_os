use alloc::prelude::v1::Box;

use crate::filesystem::result::IoResult;
use crate::multitasking::{ExplicitEventId, WaitFor, SCHEDULER};

/// Cross-process wait condition flag for file descriptor operations
#[derive(Debug, PartialEq, Eq)]
pub struct FdWaitFlag {
    event: Option<ExplicitEventId>,
}
impl FdWaitFlag {
    /// No need to wait for the first access
    pub fn new_available() -> Self {
        Self { event: None }
    }

    /// No need to wait for the first access
    pub fn new_unavailable() -> Self {
        Self {
            event: Some(WaitFor::new_event_id()),
        }
    }

    /// Called when data comes available, i.e. there is no need to wait for it anymore
    pub fn set_available<T>(&mut self, and_then: IoResult<T>) -> IoResult<T> {
        if let Some(event_id) = self.event.take() {
            log::trace!("set_available trigger {:?}", event_id);
            and_then.with_event(event_id)
        } else {
            log::trace!("set_available no trigger");
            and_then
        }
    }

    /// Called when there is no more data available
    pub fn set_unavailable(&mut self) {
        log::trace!("set_unavailable");
        if self.event.is_none() {
            self.event = Some(WaitFor::new_event_id());
        }
    }

    /// Soft-block until data comes available, or continue if it already is.
    pub fn wait(&mut self) -> WaitFor {
        if let Some(event_id) = self.event {
            WaitFor::Event(event_id)
        } else {
            WaitFor::None
        }
    }

    /// Like wait but checks that we actually wait for something
    pub fn expect_wait(&mut self) -> WaitFor {
        let event_id = self.event.expect("Data already available");
        WaitFor::Event(event_id)
    }
}
