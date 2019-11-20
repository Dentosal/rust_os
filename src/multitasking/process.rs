use alloc::prelude::v1::Vec;
use x86_64::structures::idt::{InterruptStackFrameValue, PageFaultErrorCode};
use x86_64::{PhysAddr, VirtAddr};

use crate::memory::paging::PageMap;
use crate::memory::prelude::*;

use super::ProcessId;

#[derive(Debug, Clone)]
pub struct ProcessMetadata {
    pub id: ProcessId,
    pub parent: Option<ProcessId>,
    pub status: Status,
}

#[derive(Debug, Clone)]
pub enum Status {
    /// The process is currently running
    Running,
    /// The process was terminated
    Terminated(ProcessResult),
}

#[derive(Debug, Clone)]
pub enum ProcessResult {
    /// The process exited with a return code
    Completed(u64),
    /// The process was terminated because an error occurred
    Failed(Error),
}

#[derive(Debug, Clone)]
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

/// # A suspeneded process
/// Most details of a process are stored on the stack of the process,
/// so they are not included here
#[derive(Debug, Clone)]
pub struct Process {
    /// Physical address of page tables
    pub page_table: PageMap,
    /// Stack pointer in process address space
    pub stack_pointer: VirtAddr,
    /// Stack frames
    pub stack_frames: Vec<PhysFrame>,
    /// Dynamic memory frames
    pub dynamic_memory_frames: Vec<PhysFrame>,
    /// Metadata used for scheduling etc.
    metadata: ProcessMetadata,
}
impl Process {
    pub const fn new(
        id: ProcessId, parent: Option<ProcessId>, page_table: PageMap, stack_pointer: VirtAddr,
        stack_frames: Vec<PhysFrame>,
    ) -> Self
    {
        Self {
            page_table,
            stack_pointer,
            stack_frames,
            dynamic_memory_frames: Vec::new(),
            metadata: ProcessMetadata {
                id,
                parent,
                status: Status::Running,
            },
        }
    }

    pub fn metadata(&self) -> ProcessMetadata {
        self.metadata.clone()
    }

    pub fn id(&self) -> ProcessId {
        self.metadata.id
    }
}
