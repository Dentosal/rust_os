use alloc::prelude::v1::*;
use core::convert::*;
use core::fmt;

pub use d7abi::SyscallErrorCode as ErrorCode;

use crate::multitasking::{ExplicitEventId, WaitFor};

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
}

impl<T> IoResult<T> {
    pub fn convert<A>(self) -> IoResult<A> {
        match self {
            Self::Success(v) => unreachable!("Success is never an error"),
            Self::RepeatAfter(wf) => IoResult::RepeatAfter(wf),
            Self::Code(ec) => IoResult::Code(ec),
            Self::TriggerEvent(eeid, inner) => {
                IoResult::TriggerEvent(eeid, unimplemented!("Event trigger not handled"))
            },
        }
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
