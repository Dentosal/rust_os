use serde::{Serialize, Deserialize};
use x86_64::structures::idt::InterruptStackFrameValue;
use x86_64::VirtAddr;
use x86_64::structures::idt::PageFaultErrorCode;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ProcessResult {
    /// The process exited with a return code
    Completed(u64),
    /// The process was terminated because an error occurred
    Failed(Error),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Error {
    /// Division by zero
    DivideByZero(InterruptStackFrameValue),
    /// Page fault
    PageFault(InterruptStackFrameValue, VirtAddr, PageFaultErrorCode),
    /// Unhandled interrupt without an error code
    Interrupt(u8, InterruptStackFrameValue),
    /// Unhandled interrupt with an error code
    InterruptWithCode(u8, InterruptStackFrameValue, u32),
    /// Invalid system call number
    SyscallNumber(u64),
    /// Invalid pointer passed to system call
    Pointer(VirtAddr),
}
