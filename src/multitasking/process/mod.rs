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

mod affinity;
mod create;

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
    /// Pending system call for repeating IO operations after waking up.
    /// This is set if the system call returns [`SyscallResult::RepeatAfter`].
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
        self::create::create_process(pid, args, elf)
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
