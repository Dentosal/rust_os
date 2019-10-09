use super::ProcessId;

/// TODO: use XSave instead of the current

// #[repr(C, packed, align="8")]
// struct XSaveData([u8; 1024]);

struct SavedRegisters {
    rax: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
}
impl SavedRegisters {
    pub const fn zero() -> SavedRegisters {
        SavedRegisters {
            rax: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
        }
    }
}

#[derive(Clone)]
pub struct ProcessMetadata {
    pub id: ProcessId,
    pub parent: Option<ProcessId>,
}

pub struct Process {
    // xsave: XSaveData,
    registers: SavedRegisters,
    // memory_map: Vec<memory::Block>,
    metadata: ProcessMetadata,
}
impl Process {
    pub const fn new(id: ProcessId, parent: Option<ProcessId>) -> Process {
        Process {
            registers: SavedRegisters::zero(),
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
