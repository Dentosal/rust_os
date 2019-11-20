use core::ptr;
use x86_64::structures::idt::{InterruptStackFrame, InterruptStackFrameValue, PageFaultErrorCode};
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{PhysAddr, VirtAddr};

use alloc::prelude::v1::String;

use crate::memory;
use crate::memory::paging::PageMap;
use crate::memory::prelude::*;
use crate::memory::Area;
use crate::memory::PROCESS_STACK;
use crate::multitasking::{process, Process, ProcessId, PROCMAN, SCHEDULER};

#[derive(Debug, Clone)]
pub enum SyscallResult {
    /// Return value to caller
    Success(u64),
    /// Return error to caller
    Error(u64),
    /// Terminate current process with status
    Terminate(process::ProcessResult),
}

#[must_use]
pub fn syscall(
    m: &mut memory::MemoryController, process: &mut Process, routine: u64,
    args: (u64, u64, u64, u64),
) -> SyscallResult
{
    match routine {
        // exit
        0x00 => SyscallResult::Terminate(process::ProcessResult::Completed(args.0)),
        // get_pid
        0x01 => SyscallResult::Success(process.id().as_u64()),
        // print_string
        0x02 => {
            let (str_len, str_ptr_u64, _, _) = args;
            let str_ptr = VirtAddr::new(str_ptr_u64);
            let area = Area::new_containing_block(str_ptr, str_len);
            let str_offset: u64 = str_ptr_u64 - area.start.as_u64();
            if let Some(pmap) = m.process_map_area(process, area, false) {
                let slice: &[u8] = unsafe {
                    core::slice::from_raw_parts(
                        (pmap.start + str_offset).as_ptr(),
                        str_len as usize,
                    )
                };
                let string = core::str::from_utf8(slice).expect("!UTF-8?");
                rprintln!("[pid={}] {}", process.id().as_u64(), string);
                SyscallResult::Success(0)
            } else {
                SyscallResult::Terminate(process::ProcessResult::Failed(process::Error::Pointer(
                    str_ptr,
                )))
            }
        },
        // mem_set_size
        0x03 => {
            let (size_bytes, _, _, _) = args;
            match m.process_set_dynamic_memory(process, size_bytes) {
                Some(total_bytes) => SyscallResult::Success(total_bytes),
                _ => unimplemented!("OutOfMemory case not implmented yet"),
            }
        },
        // Invalid system call number
        n => SyscallResult::Terminate(process::ProcessResult::Failed(
            process::Error::SyscallNumber(n),
        )),
    }
}

#[must_use]
pub fn handle_syscall(
    pid: ProcessId, page_table: PhysAddr, process_stack: VirtAddr,
    stack_frame: InterruptStackFrameValue,
) -> Option<process::ProcessResult>
{
    PROCMAN.update(|pm| {
        let process = pm.process_by_id_mut(pid).expect("Process not found");

        // Map process stack
        memory::configure(|mm| {
            let stack_area = mm.alloc_virtual_area(PROCESS_STACK_SIZE_PAGES);

            // Map the process stack frames to the kernel page tables
            for (page_index, frame) in process.stack_frames.iter().enumerate() {
                let vaddr = stack_area.start + (page_index as u64) * PAGE_SIZE_BYTES;
                unsafe {
                    mm.page_map
                        .map_to(
                            PT_VADDR,
                            Page::from_start_address(vaddr).unwrap(),
                            frame.clone(),
                            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                        )
                        .flush();
                }
            }

            // Retrieve required register values from the process stack
            let process_offset_from_end = PROCESS_STACK_END - process_stack;
            let stack_addr = stack_area.end - process_offset_from_end;
            let stack_ptr: *mut u64 = stack_addr.as_mut_ptr();
            let reg_rax: u64 = unsafe { ptr::read(stack_ptr.add(0)) };
            let reg_rdi: u64 = unsafe { ptr::read(stack_ptr.add(5)) };
            let reg_rsi: u64 = unsafe { ptr::read(stack_ptr.add(4)) };
            let reg_rdx: u64 = unsafe { ptr::read(stack_ptr.add(3)) };
            let reg_rcx: u64 = unsafe { ptr::read(stack_ptr.add(2)) };

            let res = match syscall(mm, process, reg_rax, (reg_rdi, reg_rsi, reg_rdx, reg_rcx)) {
                SyscallResult::Success(v) => unsafe {
                    ptr::write(stack_ptr.add(0), 1); // Success
                    ptr::write(stack_ptr.add(5), v); // Value
                    None
                },
                SyscallResult::Error(v) => unsafe {
                    ptr::write(stack_ptr.add(0), 0); // Error
                    ptr::write(stack_ptr.add(5), v); // Value
                    None
                },
                SyscallResult::Terminate(r) => Some(r),
            };

            // Unmap from the kernel tables
            for (page_index, frame) in process.stack_frames.iter().enumerate() {
                let vaddr = stack_area.start + (page_index as u64) * PAGE_SIZE_BYTES;
                unsafe {
                    mm.page_map
                        .unmap(PT_VADDR, Page::from_start_address(vaddr).unwrap())
                        .flush();
                }
            }

            // Free virtual memory
            mm.free_virtual_area(stack_area);

            res
        })
    })
}
