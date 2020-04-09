use alloc::prelude::v1::*;
use core::ptr;
use serde::{Deserialize, Serialize};
use x86_64::structures::idt::{InterruptStackFrameValue, PageFaultErrorCode};
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{PhysAddr, VirtAddr};

pub use d7abi::process::{Error, ProcessId, ProcessResult};

use crate::memory::paging::PageMap;
use crate::memory::prelude::*;
use crate::memory::process_common_code as pcc;
use crate::memory::MemoryController;
use crate::memory::{PROCESS_COMMON_CODE, PROCESS_STACK};
use crate::util::elf_parser;

use super::loader::ElfImage;

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
    /// Pending system call for repeating IO operations after waking up
    pub repeat_syscall: bool,
    /// Metadata used for scheduling etc.
    metadata: ProcessMetadata,
}
impl Process {
    pub const fn new(
        id: ProcessId, page_table: PageMap, stack_pointer: VirtAddr, stack_frames: Vec<PhysFrame>,
    ) -> Self {
        Self {
            page_table,
            stack_pointer,
            stack_frames,
            dynamic_memory_frames: Vec::new(),
            repeat_syscall: false,
            metadata: ProcessMetadata {
                id,
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

    /// Creates a new process
    pub unsafe fn create(mm: &mut MemoryController, pid: ProcessId, elf: ElfImage) -> Self {
        create_process(mm, pid, elf)
    }
}

/// Creates a new process
/// This function:
/// * Creates a stack for the new process, and populates it for returning to the process
/// * Creates a page table for the new process, and populates it with required kernel data
/// * Loads executable from an ELF image
/// Requires that the kernel page table is active.
/// Returns ProcessId and PageMap for the process.
unsafe fn create_process(mm: &mut MemoryController, pid: ProcessId, elf: ElfImage) -> Process {
    // Load image
    let (elf_header, elf_frames) = unsafe { mm.load_elf(elf) };

    // Allocate a stack for the process
    let stack_frames = mm.alloc_frames(PROCESS_STACK_SIZE_PAGES as usize);
    let stack_area = mm.alloc_virtual_area(PROCESS_STACK_SIZE_PAGES);

    // Set rsp
    // Offset to leave registers zero when they are popped,
    // plus space for the return address and other iretq data
    let registers_popped = 15; // process_common.asm : push_all
    let inthandler_tmpvar = 1;
    let iretq_structure = 5;
    let stack_items = registers_popped + inthandler_tmpvar + iretq_structure;
    let stack_size_bytes = PROCESS_STACK_SIZE_PAGES * PAGE_SIZE_BYTES;
    let stack_offset = stack_size_bytes - 8 * stack_items;
    let stack_end = PROCESS_STACK + stack_size_bytes;
    let rsp: VirtAddr = PROCESS_STACK + stack_offset;

    // Populate the process stack
    for (page_index, frame) in stack_frames.iter().enumerate() {
        let vaddr = stack_area.start + (page_index as u64) * PAGE_SIZE_BYTES;
        unsafe {
            // Map the actual stack frames to the kernel page tables
            mm.page_map
                .map_to(
                    PT_VADDR,
                    Page::from_start_address(vaddr).unwrap(),
                    frame.clone(),
                    Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                )
                .flush();

            // Zero the stack
            ptr::write_bytes(vaddr.as_mut_ptr::<u8>(), 0, frame.size() as usize);

            // Set start address to the right position, on the last page
            if page_index == (PROCESS_STACK_SIZE_PAGES as usize) - 1 {
                // Push interrupt stack frame for
                // https://os.phil-opp.com/returning-from-exceptions/#returning-from-exceptions

                macro_rules! qwords_from_end {
                    ($n:literal) => {
                        vaddr
                            .as_mut_ptr::<u64>()
                            .add((PAGE_SIZE_BYTES as usize) / 8 - $n)
                    };
                }

                // SS
                ptr::write(qwords_from_end!(1), 0);
                // RSP
                ptr::write(qwords_from_end!(2), stack_end.as_u64());
                // RFLAGFS: Interrupt flag on (https://en.wikipedia.org/wiki/FLAGS_register#FLAGS)
                ptr::write(qwords_from_end!(3), 1 << 9);
                // CS
                ptr::write(qwords_from_end!(4), 0x8u64);
                // RIP
                ptr::write(qwords_from_end!(5), elf_header.program_entry_pos);
            }

            // Unmap from kernel table
            mm.page_map
                .unmap(PT_VADDR, Page::from_start_address(vaddr).unwrap())
                .flush();
        }
    }

    // Allocate own page table for the process
    let pt_frame = mm.alloc_frames(1)[0];

    // Mapping in the kernel space
    let pt_area = mm.alloc_virtual_area(1);

    // Map table to kernel space
    unsafe {
        mm.page_map
            .map_to(
                PT_VADDR,
                Page::from_start_address(pt_area.start).unwrap(),
                pt_frame,
                Flags::PRESENT | Flags::WRITABLE,
            )
            .flush();
    }

    // Populate the page table of the process
    let mut pm = unsafe { PageMap::init(pt_area.start, pt_frame.start_address(), pt_area.start) };

    // Map the required kernel structures into the process tables
    unsafe {
        // Descriptor tables
        pm.map_to(
            pt_area.start,
            Page::from_start_address(VirtAddr::new_unchecked(0x0)).unwrap(),
            PhysFrame::from_start_address(PhysAddr::new(pcc::PROCESS_IDT_PHYS_ADDR)).unwrap(),
            // Flags::PRESENT | Flags::NO_EXECUTE,
            // CPU likes to write to GDT(?) for some reason?
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        )
        .ignore();

        // Common section for process switches
        pm.map_to(
            pt_area.start,
            Page::from_start_address(PROCESS_COMMON_CODE).unwrap(),
            PhysFrame::from_start_address(PhysAddr::new(pcc::COMMON_ADDRESS_PHYS)).unwrap(),
            Flags::PRESENT,
        )
        .ignore();

        // TODO: Rest of the structures? Are there any?
    }

    // Map process stack its own page table
    // No guard page is needed, as the page below the stack is read-only
    for (page_index, frame) in stack_frames.iter().enumerate() {
        let vaddr = PROCESS_STACK + (page_index as u64) * PAGE_SIZE_BYTES;
        unsafe {
            pm.map_to(
                pt_area.start,
                Page::from_start_address(vaddr).unwrap(),
                *frame,
                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
            )
            .ignore();
        }
    }

    // Map the executable image to its own page table
    for (ph, frames) in elf_frames {
        assert!(ph.virtual_address >= 0x400_000);
        let start = VirtAddr::new(ph.virtual_address);

        let mut flags = Flags::PRESENT;
        if !ph.has_flag(elf_parser::ELFPermissionFlags::EXECUTABLE) {
            flags |= Flags::NO_EXECUTE;
        }
        if !ph.has_flag(elf_parser::ELFPermissionFlags::READABLE) {
            panic!("Non-readable pages are not supported (yet)");
        }
        if ph.has_flag(elf_parser::ELFPermissionFlags::WRITABLE) {
            flags |= Flags::WRITABLE;
        }

        for (i, frame) in frames.into_iter().enumerate() {
            let page = Page::from_start_address(start + PAGE_SIZE_BYTES * (i as u64)).unwrap();
            unsafe {
                pm.map_to(pt_area.start, page, frame, flags).ignore();
            }
        }
    }

    // TODO: Unmap process structures from kernel page map
    // ^ at least the process page table is not unmapped yet

    Process::new(pid, pm, rsp, stack_frames)
}
