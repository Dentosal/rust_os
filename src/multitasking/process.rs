use alloc::string::String;
use alloc::vec::Vec;
use core::intrinsics::copy_nonoverlapping;
use core::ptr;
use x86_64::structures::idt::{InterruptStackFrameValue, PageFaultErrorCode};
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{PhysAddr, VirtAddr};

pub use d7abi::process::{Error, ProcessId, ProcessResult};

use crate::memory::paging::PageMap;
use crate::memory::process_common_code as pcc;
use crate::memory::MemoryController;
use crate::memory::{phys_to_virt, prelude::*};
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
    fn new(
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

    /// Creates a new process
    pub unsafe fn create(
        mm: &mut MemoryController, pid: ProcessId, args: &[String], elf: ElfImage,
    ) -> Self {
        create_process(mm, pid, args, elf)
    }

    pub fn metadata(&self) -> ProcessMetadata {
        self.metadata.clone()
    }

    pub fn id(&self) -> ProcessId {
        self.metadata.id
    }

    /// Kernel page tables must be active when this is called.
    /// Tables will be flushed after the parameter function has been called.
    pub unsafe fn modify_tables<F, R>(&mut self, mm: &mut MemoryController, f: F) -> R
    where F: FnOnce(&mut PageMap, VirtAddr) -> R {
        // Mapping in the kernel space
        let pt_area = mm.alloc_virtual_area(1);

        // Map table to kernel space
        mm.page_map
            .map_to(
                PT_VADDR,
                Page::from_start_address(pt_area.start).unwrap(),
                PhysFrame::from_start_address(self.page_table.phys_addr).unwrap(),
                Flags::PRESENT | Flags::WRITABLE,
            )
            .flush();

        let result = f(&mut self.page_table, pt_area.start);

        // Unmap the process page table from the kernel page tables
        mm.unmap_area(pt_area);

        result
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
    mm: &mut MemoryController, pid: ProcessId, args: &[String], elf: ElfImage,
) -> Process {
    // Load image
    let (elf_header, elf_frames) = unsafe { mm.load_elf(elf) };

    // Allocate a stack for the process
    let stack_frames = mm.alloc_frames(PROCESS_STACK_SIZE_PAGES as usize);
    let stack_start_phys = stack_frames.first().unwrap().start_address();
    let stack_start = phys_to_virt(stack_start_phys);
    let stack_size_bytes = (PROCESS_STACK_SIZE_PAGES * PAGE_SIZE_BYTES) as usize;

    // Zero the stack
    ptr::write_bytes(stack_start.as_mut_ptr::<u8>(), 0, stack_size_bytes);

    // Calculate offsets
    // Offset to leave registers zero when they are popped,
    // plus space for the return address and other iretq data
    let args_size_in_memory: usize =
        8 + 8 * args.len() + args.iter().map(|a| a.len()).sum::<usize>();
    let registers_popped: usize = 15; // process_common.asm : push_all
    let inthandler_tmpvar = 1;
    let iretq_structure = 5;
    let stack_items_fixed = registers_popped + inthandler_tmpvar + iretq_structure;
    let process_stack_end = PROCESS_STACK + stack_size_bytes - args_size_in_memory;
    let process_init_rsp = process_stack_end - (stack_items_fixed * 8);

    assert!(
        args_size_in_memory + stack_items_fixed <= stack_size_bytes,
        "Attempting to have too large argv"
    );

    // Populate the process stack
    unsafe {
        // Push interrupt stack frame for
        // https://os.phil-opp.com/returning-from-exceptions/#returning-from-exceptions

        let mut ptr_stack_top: *mut u8 = (stack_start + stack_size_bytes).as_mut_ptr();

        macro_rules! push_u64 {
            ($val:expr) => {
                ptr_stack_top = ptr_stack_top.sub(8);
                *(ptr_stack_top as *mut u64) = $val;
            };
        }

        macro_rules! push_u8 {
            ($val:expr) => {
                ptr_stack_top = ptr_stack_top.sub(1);
                *ptr_stack_top = $val;
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

        // Write fixed iretq structure

        // SS
        push_u64!(0);
        // RSP
        push_u64!(process_stack_end.as_u64());
        // RFLAGFS: Interrupt flag on (https://en.wikipedia.org/wiki/FLAGS_register#FLAGS)
        push_u64!(1 << 9);
        // CS
        push_u64!(0x8u64);
        // RIP
        push_u64!(elf_header.program_entry_pos);
    }

    // TODO: do processes need larger-than-one-page page tables?
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
            Page::from_start_address(VirtAddr::new_unsafe(0x0)).unwrap(),
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

    // Unmap the process page table from the kernel page tables
    mm.unmap_area(pt_area);

    // TODO: Unmap process structures from kernel page map (if any?)

    Process::new(pid, pm, process_init_rsp, stack_frames)
}

/// Loads elf image to ram and returns it
pub fn load_elf(mem_ctrl: &mut MemoryController, bytes: &[u8]) -> ElfImage {
    use core::ptr;
    use x86_64::structures::paging::PageTableFlags as Flags;

    use crate::memory::prelude::*;
    use crate::memory::Area;
    use crate::memory::{self, Page};

    let size_pages =
        memory::page_align(PhysAddr::new(bytes.len() as u64), true).as_u64() / Page::SIZE;

    // Allocate load buffer
    let area = mem_ctrl.alloc_pages(size_pages as usize, Flags::PRESENT | Flags::WRITABLE);

    unsafe {
        copy_nonoverlapping(bytes.as_ptr(), area.start.as_mut_ptr(), bytes.len());
    }

    let elf = unsafe { ElfImage::new(area) };
    elf.verify();
    elf
}
