use alloc::prelude::v1::*;
use core::convert::*;
use core::fmt;

pub use d7abi::SyscallErrorCode as ErrorCode;

use crate::multitasking::{ExplicitEventId, Scheduler, WaitFor};

use super::FileClientId;

#[derive(Debug)]
#[must_use]
pub enum IoResult<T> {
    /// The operation was successful
    Success(T),
    /// Repeat the io operation after the specified condition
    /// is fullfilled. When this result is generated while in
    /// a system call, the condition should be handled by the
    /// scheduler, so that the process sleeps before retrying.
    RepeatAfter(WaitFor),
    /// An actual error
    Code(ErrorCode),
    /// Triggers an explicit event, and then uses inner result.
    /// This can be nested multiple times.
    TriggerEvent(ExplicitEventId, Box<IoResult<T>>),
}

impl<T: fmt::Debug> IoResult<T> {
    /// Unwraps a result, yielding the content of Success
    #[inline]
    pub fn unwrap(self) -> T {
        match self {
            Self::Success(t) => t,
            error => panic!("called `unwrap` on on-success value: {:?}", error),
        }
    }

    /// Unwraps a result, yielding the contents of Success
    #[inline]
    pub fn expect(self, msg: &str) -> T {
        match self {
            Self::Success(t) => t,
            error => panic!("{}: {:?}", msg, error),
        }
    }

    /// Splits to the result itself and an optional event
    fn decompose_event(self) -> (Self, Option<ExplicitEventId>) {
        if let Self::TriggerEvent(event, t) = self {
            (*t, Some(event))
        } else {
            (self, None)
        }
    }

    /// Removes all events
    #[must_use]
    pub fn separate_events(self) -> (Self, Vec<ExplicitEventId>) {
        let mut v = self;
        let mut result = Vec::new();
        loop {
            let (new_v, event) = v.decompose_event();
            if let Some(event) = event {
                result.push(event);
            } else {
                return (new_v, result);
            }
            v = new_v;
        }
    }

    /// Processes and removes all evennts
    pub fn consume_events(self, sched: &mut Scheduler) -> Self {
        let (result, events) = self.separate_events();
        for event_id in events {
            sched.on_explicit_event(event_id);
        }
        result
    }
}

impl<T> IoResult<T> {
    pub fn is_success(&self) -> bool {
        match self {
            Self::Success(_) => true,
            Self::TriggerEvent(_, _) => unimplemented!("Event trigger not handled"),
            other => false,
        }
    }

    pub fn has_events(&self) -> bool {
        match self {
            Self::TriggerEvent(_, _) => true,
            other => false,
        }
    }

    pub fn erase_type<N>(self) -> IoResult<N> {
        match self {
            Self::Success(_) => panic!("Trying to erase type from success"),
            Self::RepeatAfter(wf) => IoResult::RepeatAfter(wf),
            Self::Code(ec) => IoResult::Code(ec),
            Self::TriggerEvent(e, t) => IoResult::TriggerEvent(e, Box::new(t.erase_type())),
        }
    }

    pub fn convert<A>(self) -> IoResult<A> {
        match self {
            Self::Success(v) => unreachable!("Success is never an error"),
            Self::RepeatAfter(wf) => IoResult::RepeatAfter(wf),
            Self::Code(ec) => IoResult::Code(ec),
            Self::TriggerEvent(_, _) => unimplemented!("Event trigger not handled"),
        }
    }

    pub fn add_events(mut self, events: impl Iterator<Item = ExplicitEventId>) -> Self {
        for event in events {
            self = Self::TriggerEvent(event, Box::new(self));
        }
        self
    }
}

/// Io errors never contain a value, so the type is replaced with Missing
pub struct Missing;

impl<T> core::ops::Try for IoResult<T> {
    type Ok = T;
    type Error = IoResult<Missing>;

    fn into_result(self) -> Result<Self::Ok, Self::Error> {
        match self {
            Self::Success(v) => Ok(v),
            other => Err(other.convert()),
        }
    }

    fn from_ok(ok: Self::Ok) -> Self {
        Self::Success(ok)
    }

    fn from_error(error: Self::Error) -> Self {
        error.convert()
    }
}
