use x86_64::{PhysAddr, VirtAddr};

use super::ProcessId;

#[derive(Debug, Clone)]
pub struct ProcessMetadata {
    pub id: ProcessId,
    pub parent: Option<ProcessId>,
}

/// # A suspeneded process
/// Most details of a process are stored on the stack of the process,
/// so they are not included here
#[derive(Debug, Clone)]
pub struct Process {
    /// Physical address of page tables for this process
    pub page_table: PhysAddr,
    /// Stack pointer for this process
    pub stack_pointer: VirtAddr,
    /// Metadata used for scheduling etc.
    metadata: ProcessMetadata,
}
impl Process {
    pub const fn new(
        id: ProcessId,
        parent: Option<ProcessId>,
        page_table: PhysAddr,
        stack_pointer: VirtAddr,
    ) -> Self {
        Self {
            page_table,
            stack_pointer,
            metadata: ProcessMetadata { id, parent },
        }
    }

    pub fn metadata(&self) -> ProcessMetadata {
        self.metadata.clone()
    }

    pub fn id(&self) -> ProcessId {
        self.metadata.id
    }
}
