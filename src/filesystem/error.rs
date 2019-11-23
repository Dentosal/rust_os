pub use d7abi::SyscallErrorCode as ErrorCode;

use crate::multitasking::WaitFor;

#[must_use]
pub type IoResult<T> = Result<T, IoError>;

/// TODO: Rename. There are other things than errors here as well.
#[derive(Debug)]
#[must_use]
pub enum IoError {
    /// Repeat the io operation after the specified condition
    /// is fullfilled. When this result is generated while in
    /// a system call, the condition should be handled by the
    /// scheduler, so that the process sleeps before retrying.
    RepeatAfter(WaitFor),
    /// An actual error
    Code(ErrorCode),
}
