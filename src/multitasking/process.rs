use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::intrinsics::copy_nonoverlapping;
use core::ptr;
use x86_64::structures::idt::{InterruptStackFrameValue, PageFaultErrorCode};
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{PhysAddr, VirtAddr};

pub use d7abi::process::{Error, ProcessId, ProcessResult};

use crate::memory::paging::{PageMap, PAGE_MAP};
use crate::memory::phys::OutOfMemory;
use crate::memory::process_common_code as pcc;
use crate::memory::{phys, virt};
use crate::memory::{phys_to_virt, prelude::*};
use crate::memory::{PROCESS_COMMON_CODE, PROCESS_STACK};
use crate::util::elf_parser::{self, ELFHeader, ELFProgramHeader};

use super::ElfImage;

#[derive(Debug, Clone)]
pub struct ProcessMetadata {
    pub id: ProcessId,
    pub status: Status,
}

#[derive(Debug, Clone)]
pub enum Status {
    /// The process is currently running
    Running,
    /// The process was terminated
    Terminated(ProcessResult),
}

/// A (cheaply-)copyable subset of a process descriptor required for switching
/// into that process. Doesn't own process memory etc.
#[derive(Debug, Clone, Copy)]
pub struct ProcessSwitchInfo {
    /// Physical address of page tables
    pub p4addr: PhysAddr,
    /// Stack pointer in process address space
    pub stack_pointer: VirtAddr,
    /// Metadata used for scheduling etc.
    pub pid: ProcessId,
}

/// A process descriptor. Owns the memory of the process, among other things.
///
/// Some details of a are stored ...
/// * on the stack of the process when it's suspended
/// * in registers, when the process is running
/// so they are not included here
#[derive(Debug)]
pub struct Process {
    /// Physical address of page tables
    pub page_table: PageMap,
    /// Stack pointer in process address space
    pub stack_pointer: VirtAddr,
    /// Stack frames
    pub stack_memory: phys::Allocation,
    /// Dynamic memory frames, e.g. process heap
    pub dynamic_memory: Vec<phys::Allocation>,
    /// Pending system call for repeating IO operations after waking up
    pub repeat_syscall: bool,
    /// Metadata used for scheduling etc.
    metadata: ProcessMetadata,
}
impl Process {
    fn new(
        id: ProcessId, page_table: PageMap, stack_pointer: VirtAddr, stack_memory: phys::Allocation,
    ) -> Self {
        Self {
            page_table,
            stack_pointer,
            stack_memory,
            dynamic_memory: Vec::new(),
            repeat_syscall: false,
            metadata: ProcessMetadata {
                id,
                status: Status::Running,
            },
        }
    }

    pub fn switch_info(&self) -> ProcessSwitchInfo {
        ProcessSwitchInfo {
            p4addr: self.page_table.p4_addr(),
            stack_pointer: self.stack_pointer,
            pid: self.metadata.id,
        }
    }

    /// Creates a new process
    pub unsafe fn create(
        pid: ProcessId, args: &[String], elf: ElfImage,
    ) -> Result<Self, OutOfMemory> {
        create_process(pid, args, elf)
    }

    pub fn metadata(&self) -> ProcessMetadata {
        self.metadata.clone()
    }

    pub fn id(&self) -> ProcessId {
        self.metadata.id
    }

    /// Read u64 values from top of the stack
    pub unsafe fn read_stack_u64(&self, depth: usize) -> u64 {
        let buf = [0; 8];
        let smem = self.stack_memory.read();
        todo!();
        buf.copy_from_slice(smem);
        u64::from_ne_bytes(buf)
    }

    pub unsafe fn write_stack_u64(&self, depth: usize, value: u64) {
        let smem = self.stack_memory.read();
        todo!();
    }

    /// Map process-owned memory to a contiguous virtual address space
    /// in kernel page tables. This is done using page tables of the
    /// process.
    ///
    /// # Safety
    /// Caller must ensure that no overlapping slices are created.
    pub unsafe fn memory_slice(
        &self, ptr: VirtAddr, len: usize,
    ) -> Option<(virt::Allocation, &[u8])> {
        todo!();
        // let page_map = PAGE_MAP.lock();
        // unsafe {
        //     let proc_pt = phys_to_virt(self.page_table.phys_addr);

        //     // for (i, frame) in frames.enumerate() {
        //     //     process
        //     //         .page_table
        //     //         .map_to(
        //     //             proc_pt,
        //     //             Page::from_start_address(virt_addr + (i as u64) * PAGE_SIZE_BYTES).unwrap(),
        //     //             frame,
        //     //             flags,
        //     //         )
        //     //         .ignore();
        //     // }
        // }
    }

    /// Map process-owned memory to a contiguous virtual address space
    /// in kernel page tables. This is done using page tables of the
    /// process.
    ///
    /// # Safety
    /// Caller must ensure that no overlapping slices are created.
    pub unsafe fn memory_slice_mut(
        &self, ptr: VirtAddr, len: usize,
    ) -> Option<(virt::Allocation, &mut [u8])> {
        todo!();
    }
}

/// Creates a new process
/// This function:
/// * Creates a stack for the new process, and populates it for returning to the process
/// * Creates a page table for the new process, and populates it with required kernel data
/// * Loads executable from an ELF image
/// Requires that the kernel page table is active.
/// Returns ProcessId and PageMap for the process.
unsafe fn create_process(
    pid: ProcessId, args: &[String], elf: ElfImage,
) -> Result<Process, OutOfMemory> {
    // Allocate a stack for the process
    let stack_size_bytes = (PROCESS_STACK_SIZE_PAGES * PAGE_SIZE_BYTES) as usize;
    let mut stack = phys::allocate_zeroed(
        Layout::from_size_align(stack_size_bytes, PAGE_SIZE_BYTES as usize).unwrap(),
    )?; // TODO: propagate error

    // Calculate offsets
    // Offset to leave registers zero when they are popped,
    // plus space for the return address and other iretq data
    let args_size_in_memory: usize = 8
        + 8 * args.len()
        + args
            .iter()
            .map(|a| a.len())
            .sum::<usize>()
            .next_multiple_of(8);
    let registers_popped: usize = 15; // process_common.asm : push_all
    let inthandler_tmpvar = 1;
    let iretq_structure = 5;
    let stack_items_fixed = registers_popped + inthandler_tmpvar + iretq_structure;
    let process_stack_end = PROCESS_STACK + stack_size_bytes - args_size_in_memory;
    let process_init_rsp = process_stack_end - (stack_items_fixed * 8);

    log::trace!("init rsp {:p}", process_init_rsp);

    assert!(
        args_size_in_memory + stack_items_fixed <= stack_size_bytes,
        "Attempting to have too large argv"
    );


    // Populate the process stack
    {
        let mut top: usize = stack_size_bytes;
        let stack_mem = stack.write();

        macro_rules! push_u64 {
            ($val:expr) => {
                top -= 8;
                let v: u64 = $val;
                stack_mem[top..top + 8].copy_from_slice(&v.to_ne_bytes());
            };
        }

        macro_rules! align_u64 {
            () => {
                while top % 8 != 0 {
                    push_u8!(0);
                }
            };
        }

        macro_rules! push_u8 {
            ($val:expr) => {
                top -= 1;
                stack_mem[top] = $val;
            };
        }

        // Write process arguments into it's stack
        push_u64!(args.len() as u64);
        for arg in args {
            push_u64!(arg.len() as u64);
        }

        for arg in args.iter().rev() {
            for byte in arg.as_bytes().iter().rev() {
                push_u8!(*byte);
            }
        }

        align_u64!();

        // Write fixed iretq structure
        // https://os.phil-opp.com/returning-from-exceptions/#returning-from-exceptions

        // SS
        push_u64!(0);
        // RSP
        push_u64!(process_stack_end.as_u64());
        // RFLAGFS: Interrupt flag on (https://en.wikipedia.org/wiki/FLAGS_register#FLAGS)
        push_u64!(0x0202);
        // CS
        push_u64!(0x8u64);
        // RIP
        push_u64!(elf.header.program_entry_pos);
    }

    // TODO: do processes need larger-than-one-page page tables?
    // Allocate own page table for the process
    let pt_frame = phys::allocate(PAGE_LAYOUT)?;

    // Populate the page table of the process
    let mut pm = unsafe {
        PageMap::init(
            pt_frame.mapped_start(),
            pt_frame.phys_start(),
            pt_frame.mapped_start(), // TODO: is this correct?
        )
    };

    // Map the required kernel structures into the process tables
    unsafe {
        // Descriptor tables
        pm.map_to(
            pt_frame.mapped_start(),
            Page::from_start_address(VirtAddr::new_unsafe(0x0)).unwrap(),
            PhysFrame::from_start_address(PhysAddr::new(pcc::PROCESS_IDT_PHYS_ADDR)).unwrap(),
            // FIXME: ACCESSED flag should mean that CPU will not attempt to write to this?
            Flags::PRESENT | Flags::NO_EXECUTE | Flags::ACCESSED | Flags::WRITABLE,
        )
        .ignore();

        // Common section for process switches
        pm.map_to(
            pt_frame.mapped_start(),
            Page::from_start_address(PROCESS_COMMON_CODE).unwrap(),
            PhysFrame::from_start_address(PhysAddr::new(pcc::COMMON_ADDRESS_PHYS)).unwrap(),
            Flags::PRESENT,
        )
        .ignore();

        // TODO: Rest of the structures? Are there any?
    }

    // Map process stack its own page table
    // No guard page is needed, as the page below the stack is read-only
    for i in 0..PROCESS_STACK_SIZE_PAGES {
        unsafe {
            pm.map_to(
                pt_frame.mapped_start(),
                Page::from_start_address(PROCESS_STACK + i * PAGE_SIZE_BYTES).unwrap(),
                PhysFrame::from_start_address(stack.phys_start()  + i * PAGE_SIZE_BYTES).unwrap(),
                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
            )
            .ignore();
        }
    }

    // Map the executable image to its own page table
    for (ph, frames) in elf.sections {
        assert!(ph.virtual_address >= 0x400_000);
        let start = VirtAddr::new(ph.virtual_address);

        let mut flags = Flags::PRESENT;
        if !ph.has_flag(elf_parser::ELFPermissionFlags::EXECUTABLE) {
            flags |= Flags::NO_EXECUTE;
        }
        if !ph.has_flag(elf_parser::ELFPermissionFlags::READABLE) {
            panic!("Non-readable pages are not supported (yet?)");
        }
        if ph.has_flag(elf_parser::ELFPermissionFlags::WRITABLE) {
            flags |= Flags::WRITABLE;
        }

        for (i, frame) in frames.into_iter().enumerate() {
            // TODO: assumes that frames are page-sized
            let page = Page::from_start_address(start + PAGE_SIZE_BYTES * (i as u64)).unwrap();
            unsafe {
                pm.map_to(
                    pt_frame.mapped_start(),
                    page,
                    PhysFrame::from_start_address(frame.phys_start()).unwrap(),
                    flags,
                )
                .ignore();
            }
        }
    }

    Ok(Process::new(pid, pm, process_init_rsp, stack))
}
