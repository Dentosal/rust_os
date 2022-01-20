use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::intrinsics::copy_nonoverlapping;
use core::ptr;
use d7abi::{MemoryProtectionFlags, SyscallErrorCode};
use x86_64::structures::idt::{InterruptStackFrameValue, PageFaultErrorCode};
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{align_down, align_up, PhysAddr, VirtAddr};

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
    /// Elf image RAII guard
    /// TODO: have a common pool for these, so they can be shared and reused
    _elf_image: ElfImage,
    /// Metadata used for scheduling etc.
    metadata: ProcessMetadata,
}
impl Process {
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

    /// Read u64 values from top of the stack.
    /// Panics if `depth` would go beyond the stack area.
    pub fn read_stack_u64(&self, depth: usize) -> u64 {
        let smem = self.stack_memory.read();
        let top = (self.stack_pointer - PROCESS_STACK) as usize;
        let item = top + depth * 8;
        let mut buf = [0; 8];
        buf.copy_from_slice(&smem[item..item + 8]);
        u64::from_ne_bytes(buf)
    }

    /// Panics if `depth` would go beyond the stack area.
    pub fn write_stack_u64(&mut self, depth: usize, value: u64) {
        let smem = self.stack_memory.write();
        let top = (self.stack_pointer - PROCESS_STACK) as usize;
        let item = top + depth * 8;
        smem[item..item + 8].copy_from_slice(&value.to_ne_bytes());
    }

    /// Map process-owned memory to a contiguous virtual address space
    /// in kernel page tables. This is done using page tables of the
    /// process.
    ///
    /// # Safety
    /// Caller must ensure that no overlapping slices are created.
    pub unsafe fn memory_slice(
        &mut self, ptr: VirtAddr, len: usize,
    ) -> Option<(virt::Allocation, &[u8])> {
        log::trace!(
            "Reading process memory at {:x}..{:x} (len={:x})",
            ptr,
            ptr + len,
            len
        );

        let mut page_map = PAGE_MAP.lock();

        let flags = Flags::PRESENT;
        unsafe {
            let proc_pt_vaddr = phys_to_virt(self.page_table.phys_addr);

            let r_start = align_down(ptr.as_u64(), PAGE_SIZE_BYTES);
            let r_end = align_up(ptr.as_u64() + (len as u64), PAGE_SIZE_BYTES);
            let r_size_pages = (r_end - r_start) / PAGE_SIZE_BYTES;

            let offset = ptr.as_u64() - r_start;
            let virtarea = virt::allocate(r_size_pages as usize);

            for i in 0..r_size_pages {
                let r_offset = i * PAGE_SIZE_BYTES;
                let proc_frame_start = VirtAddr::new(r_start + r_offset);
                let page = Page::from_start_address(virtarea.start + r_offset).unwrap();

                let phys_addr = self.page_table.translate(proc_pt_vaddr, proc_frame_start)?;

                page_map
                    .map_to(
                        PT_VADDR,
                        page,
                        PhysFrame::from_start_address_unchecked(phys_addr),
                        flags,
                    )
                    .ignore();
            }

            let slice: &[u8] = core::slice::from_raw_parts((virtarea.start + offset).as_ptr(), len);
            Some((virtarea, slice))
        }
    }

    /// Map process-owned memory to a contiguous virtual address space
    /// in kernel page tables. This is done using page tables of the
    /// process.
    ///
    /// # Safety
    /// Caller must ensure that no overlapping slices are created.
    pub unsafe fn memory_slice_mut(
        &mut self, ptr: VirtAddr, len: usize,
    ) -> Option<(virt::Allocation, &mut [u8])> {
        log::trace!(
            "Writing process memory at {:x}..{:x} (len={:x})",
            ptr,
            ptr + len,
            len
        );

        let mut page_map = PAGE_MAP.lock();

        let flags = Flags::PRESENT | Flags::WRITABLE;
        unsafe {
            let proc_pt_vaddr = phys_to_virt(self.page_table.phys_addr);

            let r_start = align_down(ptr.as_u64(), PAGE_SIZE_BYTES);
            let r_end = align_up(ptr.as_u64() + (len as u64), PAGE_SIZE_BYTES);
            let r_size_pages = (r_end - r_start) / PAGE_SIZE_BYTES;

            let offset = ptr.as_u64() - r_start;
            let virtarea = virt::allocate(r_size_pages as usize);

            for i in 0..r_size_pages {
                let r_offset = i * PAGE_SIZE_BYTES;
                let proc_frame_start = VirtAddr::new(r_start + r_offset);
                let page = Page::from_start_address(virtarea.start + r_offset).unwrap();

                let phys_addr = self.page_table.translate(proc_pt_vaddr, proc_frame_start)?;

                page_map
                    .map_to(
                        PT_VADDR,
                        page,
                        PhysFrame::from_start_address_unchecked(phys_addr),
                        flags,
                    )
                    .ignore();
            }

            let slice: &mut [u8] =
                core::slice::from_raw_parts_mut((virtarea.start + offset).as_mut_ptr(), len);
            Some((virtarea, slice))
        }
    }

    /// Allocate some memory for the process,
    /// or change flags of an already allocated block
    pub fn memory_alloc(
        &mut self, area_ptr: VirtAddr, size: usize, flags: MemoryProtectionFlags,
    ) -> Result<(), SyscallErrorCode> {
        if area_ptr.as_u64() % PAGE_SIZE_BYTES != 0 {
            log::warn!(
                "Memory allocation failed: incorrect area aligment {:x}",
                area_ptr.as_u64()
            );
            return Err(SyscallErrorCode::mmap_incorrect_alignment);
        }

        if size as u64 % PAGE_SIZE_BYTES != 0 {
            log::warn!(
                "Memory allocation failed: incorrect size aligment {:x}",
                size
            );
            return Err(SyscallErrorCode::mmap_incorrect_alignment);
        }

        let size_pages = size / (PAGE_SIZE_BYTES as usize);

        let mut pt_flags = Flags::PRESENT;
        if !flags.contains(MemoryProtectionFlags::READ) {
            // TODO: unreadable mappings?
            return Err(SyscallErrorCode::unsupported);
        }
        if flags.contains(MemoryProtectionFlags::WRITE) {
            pt_flags |= Flags::WRITABLE;
        }
        if !flags.contains(MemoryProtectionFlags::EXECUTE) {
            pt_flags |= Flags::NO_EXECUTE;
        }

        let proc_pt_vaddr = phys_to_virt(self.page_table.phys_addr);

        for i in 0..size_pages {
            let offset = i * (PAGE_SIZE_BYTES as usize);
            let proc_frame_start = area_ptr + offset;

            log::debug!("mem_alloc: checking {:p}", proc_frame_start);

            if let Some(phys_start) =
                unsafe { self.page_table.translate(proc_pt_vaddr, proc_frame_start) }
            {
                // Already mapped, check that it's a dynamically mapped region.
                // Otherwise the process is not allowed to change it.
                let is_dynamic = self
                    .dynamic_memory
                    .iter()
                    .any(|b| phys_start == unsafe { b.phys_start() });

                if !is_dynamic {
                    log::warn!("Memory allocation failed: permission denied");
                    return Err(SyscallErrorCode::mmap_permission_error);
                }

                log::debug!("mem_alloc: only set flags for {:p}", proc_frame_start);

                // Permissions ok, change flags
                unsafe {
                    self.page_table
                        .map_to(
                            proc_pt_vaddr,
                            Page::from_start_address(area_ptr + offset).unwrap(),
                            PhysFrame::from_start_address_unchecked(phys_start),
                            pt_flags,
                        )
                        .ignore();
                }
            } else {
                // Not yet mapped, allocate and map
                let allocation = phys::allocate_zeroed(PAGE_LAYOUT)
                    .map_err(|OutOfMemory| SyscallErrorCode::out_of_memory)?;

                log::debug!("mem_alloc: allocate page {:p}", proc_frame_start);

                unsafe {
                    self.page_table
                        .map_to(
                            proc_pt_vaddr,
                            Page::from_start_address(area_ptr + offset).unwrap(),
                            PhysFrame::from_start_address_unchecked(allocation.phys_start()),
                            pt_flags,
                        )
                        .ignore();
                }

                self.dynamic_memory.push(allocation);
            }
        }

        Ok(())
    }

    pub fn memory_dealloc(
        &mut self, area_ptr: VirtAddr, size: usize,
    ) -> Result<(), SyscallErrorCode> {
        if area_ptr.as_u64() % PAGE_SIZE_BYTES != 0 {
            log::warn!(
                "Memory deallocation failed: incorrect area aligment {:x}",
                area_ptr.as_u64()
            );
            return Err(SyscallErrorCode::mmap_incorrect_alignment);
        }

        if size as u64 % PAGE_SIZE_BYTES != 0 {
            log::warn!(
                "Memory deallocation failed: incorrect size aligment {:x}",
                size
            );
            return Err(SyscallErrorCode::mmap_incorrect_alignment);
        }

        let size_pages = size / (PAGE_SIZE_BYTES as usize);

        let proc_pt_vaddr = phys_to_virt(self.page_table.phys_addr);

        for i in 0..size_pages {
            let offset = i * (PAGE_SIZE_BYTES as usize);
            let proc_frame_start = area_ptr + offset;
            if let Some(phys_start) =
                unsafe { self.page_table.translate(proc_pt_vaddr, proc_frame_start) }
            {
                // Check that it's a dynamically mapped region.
                // Otherwise the process is not allowed to change it.
                // This also deallocates the region by dropping it.
                let is_dynamic = self
                    .dynamic_memory
                    .drain_filter(|b| phys_start == unsafe { b.phys_start() })
                    .next()
                    .is_some();

                if !is_dynamic {
                    log::warn!("Memory deallocation failed: permission denied");
                    return Err(SyscallErrorCode::mmap_permission_error);
                }

                // Permissions ok, unmap
                unsafe {
                    self.page_table
                        .unmap(
                            proc_pt_vaddr,
                            Page::from_start_address(area_ptr + offset).unwrap(),
                        )
                        .ignore();
                }
            }
        }

        Ok(())
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
    )?;

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
    let pm_addr = pt_frame.mapped_start();
    let mut pm = unsafe {
        PageMap::init(
            pm_addr,
            pt_frame.phys_start(),
            pm_addr, // TODO: is this correct?
        )
    };
    core::mem::forget(pt_frame); // Ownership moved to the page map

    // Map the required kernel structures into the process tables
    unsafe {
        // Descriptor tables
        pm.map_to(
            pm_addr,
            Page::from_start_address(VirtAddr::new_unsafe(0x0)).unwrap(),
            PhysFrame::from_start_address(PhysAddr::new(pcc::PROCESS_IDT_PHYS_ADDR)).unwrap(),
            // FIXME: GDT.ACCESSED flag should mean that CPU will not attempt to write to this?
            Flags::PRESENT | Flags::NO_EXECUTE | Flags::WRITABLE,
        )
        .ignore();

        // Common section for process switches
        pm.map_to(
            pm_addr,
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
                pm_addr,
                Page::from_start_address(PROCESS_STACK + i * PAGE_SIZE_BYTES).unwrap(),
                PhysFrame::from_start_address(stack.phys_start() + i * PAGE_SIZE_BYTES).unwrap(),
                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
            )
            .ignore();
        }
    }

    // Map the executable image to its own page table
    for (ph, frames) in &elf.sections {
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
                    pm_addr,
                    page,
                    PhysFrame::from_start_address(frame.phys_start()).unwrap(),
                    flags,
                )
                .ignore();
            }
        }
    }

    Ok(Process {
        page_table: pm,
        stack_pointer: process_init_rsp,
        stack_memory: stack,
        dynamic_memory: Vec::new(),
        repeat_syscall: false,
        _elf_image: elf,
        metadata: ProcessMetadata {
            id: pid,
            status: Status::Running,
        },
    })
}
