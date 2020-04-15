use alloc::prelude::v1::*;
use core::convert::*;
use core::fmt;

pub use d7abi::SyscallErrorCode as ErrorCode;

use crate::multitasking::{ExplicitEventId, Scheduler, WaitFor};

use super::FileClientId;

/// IoResult without events
#[derive(Debug)]
#[must_use]
pub enum IoResultPure<T> {
    /// The operation was successful
    Success(T),
    /// Repeat the io operation after the specified condition
    /// is fullfilled. When this result is generated while in
    /// a system call, the condition should be handled by the
    /// scheduler, so that the process sleeps before retrying.
    RepeatAfter(WaitFor),
    /// An actual error
    Error(ErrorCode),
}

/// IO operation result, and associated event triggers
#[derive(Debug)]
#[must_use]
pub struct IoResult<T> {
    inner: IoResultPure<T>,
    events: Vec<ExplicitEventId>,
}

impl<T> IoResult<T> {
    pub fn success(value: T) -> Self {
        Self {
            inner: IoResultPure::Success(value),
            events: Vec::new(),
        }
    }

    pub fn error(ecode: ErrorCode) -> Self {
        Self {
            inner: IoResultPure::Error(ecode),
            events: Vec::new(),
        }
    }

    pub fn repeat_after(event: WaitFor) -> Self {
        Self {
            inner: IoResultPure::RepeatAfter(event),
            events: Vec::new(),
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self.inner, IoResultPure::Success(_))
    }

    pub fn has_events(&self) -> bool {
        self.events.is_empty()
    }

    pub fn map<N>(self, f: fn(T) -> N) -> IoResult<N> {
        IoResult {
            inner: match self.inner {
                IoResultPure::Success(v) => IoResultPure::Success(f(v)),
                IoResultPure::RepeatAfter(wf) => IoResultPure::RepeatAfter(wf),
                IoResultPure::Error(ec) => IoResultPure::Error(ec),
            },
            events: self.events,
        }
    }

    pub fn erase_type<N>(self) -> IoResult<N> {
        IoResult {
            inner: match self.inner {
                IoResultPure::Success(_) => panic!("Cannot erase type of success"),
                IoResultPure::RepeatAfter(wf) => IoResultPure::RepeatAfter(wf),
                IoResultPure::Error(ec) => IoResultPure::Error(ec),
            },
            events: self.events,
        }
    }

    pub fn with_event(mut self, event: ExplicitEventId) -> Self {
        self.events.push(event);
        self
    }

    pub fn with_events(mut self, events: impl Iterator<Item = ExplicitEventId>) -> Self {
        self.events.extend(events);
        self
    }

    pub fn with_context(mut self, mut context: IoContext) -> Self {
        self.events.extend(context.events.drain(..));
        self
    }

    pub fn separate_events(mut self) -> (IoResultPure<T>, IoContext) {
        (self.inner, IoContext {
            events: self.events,
        })
    }
}

impl<T> IoResultPure<T> {
    pub fn erase_type<N>(self) -> IoResultPure<N> {
        match self {
            Self::Success(_) => panic!("Cannot erase type of success"),
            Self::RepeatAfter(wf) => IoResultPure::RepeatAfter(wf),
            Self::Error(ec) => IoResultPure::Error(ec),
        }
    }
}

impl<T: fmt::Debug> IoResult<T> {
    /// Unwraps a result, yielding the content of Success
    #[inline]
    pub fn unwrap(self) -> T {
        assert!(
            self.events.is_empty(),
            "Called unwrap on result with pending events"
        );
        match self.inner {
            IoResultPure::Success(t) => t,
            error => panic!("called `unwrap` on on-success value: {:?}", error),
        }
    }

    /// Unwraps a result, yielding the contents of Success
    #[inline]
    pub fn expect(self, msg: &str) -> T {
        assert!(
            self.events.is_empty(),
            "Called expect on result with pending events"
        );
        match self.inner {
            IoResultPure::Success(t) => t,
            error => panic!("{}: {:?}", msg, error),
        }
    }

    /// Processes and removes all evennts
    pub fn consume_events(self, sched: &mut Scheduler) -> IoResultPure<T> {
        for event_id in self.events.into_iter() {
            sched.on_explicit_event(event_id);
        }
        self.inner
    }

    /// Panics if there are any events
    pub fn expect_events(self, msg: &str) -> IoResultPure<T> {
        if !self.events.is_empty() {
            panic!("{}: result contains events", msg);
        }
        self.inner
    }
}

impl<T: fmt::Debug> IoResultPure<T> {
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
}

/// IO operation context
#[derive(Debug)]
#[must_use = "IoContext must be handled to prevent dropping events"]
pub struct IoContext {
    events: Vec<ExplicitEventId>,
}
impl IoContext {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// No events (yet)
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn consume(&mut self, mut other: IoContext) {
        self.events.extend(other.events.drain(..));
    }
}

impl Drop for IoContext {
    fn drop(&mut self) {
        assert!(self.is_empty(), "Unhandled events in IoContext");
    }
}

/// IO errors never contain a value, so the type is replaced with Missing
pub struct Missing;

impl<T> core::ops::Try for IoResult<T> {
    type Ok = (T, IoContext);
    type Error = IoResult<Missing>;

    fn into_result(self) -> Result<Self::Ok, Self::Error> {
        if let IoResultPure::Success(v) = self.inner {
            Ok((v, IoContext {
                events: self.events,
            }))
        } else {
            Err(self.erase_type())
        }
    }

    fn from_ok((inner, mut context): Self::Ok) -> Self {
        Self {
            inner: IoResultPure::Success(inner),
            events: context.events.drain(..).collect(),
        }
    }

    fn from_error(error: Self::Error) -> Self {
        error.erase_type()
    }
}

impl<T> core::ops::Try for IoResultPure<T> {
    type Ok = T;
    type Error = IoResultPure<Missing>;

    fn into_result(self) -> Result<Self::Ok, <Self as core::ops::Try>::Error> {
        if let Self::Success(v) = self {
            Ok(v)
        } else {
            Err(self.erase_type())
        }
    }

    fn from_ok(value: Self::Ok) -> Self {
        Self::Success(value)
    }

    fn from_error(error: <Self as core::ops::Try>::Error) -> Self {
        error.erase_type()
    }
}

impl<T> From<IoResultPure<T>> for IoResult<T> {
    fn from(inner: IoResultPure<T>) -> Self {
        Self {
            inner,
            events: Vec::new(),
        }
    }
}
