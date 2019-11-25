use alloc::prelude::v1::String;
use core::convert::{From, TryFrom};
use core::mem;
use core::ptr;
use x86_64::structures::idt::{InterruptStackFrame, InterruptStackFrameValue, PageFaultErrorCode};
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{PhysAddr, VirtAddr};

use d7abi::fs::{FileDescriptor, FileInfo};

use crate::filesystem::{error::*, FILESYSTEM};
use crate::memory;
use crate::memory::paging::PageMap;
use crate::memory::prelude::*;
use crate::memory::Area;
use crate::memory::PROCESS_STACK;
use crate::multitasking::{process, Process, ProcessId, WaitFor, SCHEDULER};

#[derive(Debug, Clone)]
#[must_use]
pub enum SyscallResult {
    /// Return a value to caller
    Continue(Result<u64, u64>),
    /// Switch to another process immediately.
    /// Returns a value to the caller as well.
    /// The second argument describes scheduling
    /// conditions for the calling process.
    Switch(Result<u64, u64>, WaitFor),
    /// Switch to another process immediately.
    /// Returns a value to the caller as well.
    /// The second argument describes scheduling
    /// conditions for the calling process.
    RepeatAfter(WaitFor),
    /// Terminate current process with status
    Terminate(process::ProcessResult),
}

impl core::ops::Try for SyscallResult {
    type Ok = Self;
    type Error = IoError;

    fn into_result(self) -> Result<Self, IoError> {
        unimplemented!("??")
    }

    fn from_ok(ok: Self) -> Self {
        ok
    }

    fn from_error(error: Self::Error) -> Self {
        use crate::multitasking::process::Error::SyscallNumber;
        match error {
            IoError::RepeatAfter(waitfor) => Self::RepeatAfter(waitfor),
            IoError::Code(code) => Self::Continue(Err(code.into())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSyscall {
    pub routine: u64,
    args: (u64, u64, u64, u64),
}

pub fn syscall(
    m: &mut memory::MemoryController, process: &mut Process, rsc: RawSyscall,
) -> SyscallResult {
    use d7abi::SyscallNumber as SC;

    use crate::filesystem::FileClientId;

    if let Ok(sc) = SC::try_from(rsc.routine) {
        match sc {
            SC::exit => SyscallResult::Terminate(process::ProcessResult::Completed(rsc.args.0)),
            SC::get_pid => SyscallResult::Continue(Ok(process.id().as_u64())),
            SC::debug_print => {
                let (str_len, str_ptr_u64, _, _) = rsc.args;
                let str_ptr = VirtAddr::new(str_ptr_u64);
                let (area, slice) = unsafe {
                    match m.process_slice(process, str_len, str_ptr) {
                        Some(v) => v,
                        None => {
                            return SyscallResult::Terminate(process::ProcessResult::Failed(
                                process::Error::Pointer(str_ptr),
                            ));
                        },
                    }
                };
                let string = core::str::from_utf8(slice).expect("TODO: debug_print: Invalid UTF-8");
                rprintln!("[pid={}] {}", process.id().as_u64(), string);
                unsafe { m.unmap_area(area) };
                SyscallResult::Continue(Ok(0))
            },
            SC::mem_set_size => {
                let (size_bytes, _, _, _) = rsc.args;
                match m.process_set_dynamic_memory(process, size_bytes) {
                    Some(total_bytes) => SyscallResult::Continue(Ok(total_bytes)),
                    _ => unimplemented!("OutOfMemory case not implmented yet"),
                }
            },
            SC::fs_fileinfo => {
                let (path_len, path_ptr, dst_ptr, _) = rsc.args;
                let path_ptr = VirtAddr::new(path_ptr);
                let dst_ptr = VirtAddr::new(dst_ptr);
                if let Some((area, slice)) = unsafe { m.process_slice(process, path_len, path_ptr) }
                {
                    let path =
                        core::str::from_utf8(slice).expect("TODO: fs_fileinfo: Invalid UTF-8");
                    let mut fs = FILESYSTEM.try_lock().expect("FILESYSTEM LOCKED");
                    let fileinfo: FileInfo = fs.fileinfo(path)?;
                    unsafe {
                        m.process_write_value(process, fileinfo, dst_ptr);
                    }
                    unsafe { m.unmap_area(area) };
                    SyscallResult::Continue(Ok(0))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(path_ptr),
                    ))
                }
            },
            SC::fs_open => {
                let (path_len, path_ptr, _, _) = rsc.args;
                let path_ptr = VirtAddr::new(path_ptr);
                if let Some((area, slice)) = unsafe { m.process_slice(process, path_len, path_ptr) }
                {
                    let path =
                        core::str::from_utf8(slice).expect("TODO: fs_fileinfo: Invalid UTF-8");
                    let mut fs = FILESYSTEM.try_lock().expect("FILESYSTEM LOCKED");
                    let fc = fs.open(path, process.id())?;
                    unsafe { m.unmap_area(area) };
                    SyscallResult::Continue(Ok(unsafe { fc.fd.as_u64() }))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(path_ptr),
                    ))
                }
            },
            SC::fd_read => {
                let (fd, buf, count, _) = rsc.args;
                let fd = unsafe { FileDescriptor::from_u64(fd) };
                let buf = VirtAddr::new(buf);
                let mut fs = FILESYSTEM.try_lock().expect("FILESYSTEM LOCKED");
                if let Some((area, slice)) = unsafe { m.process_slice_mut(process, count, buf) } {
                    let fc = FileClientId::process(process.id(), fd);
                    let read_count = fs.read(fc, slice)?;
                    unsafe { m.unmap_area(area) };
                    SyscallResult::Continue(Ok(read_count as u64))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(buf),
                    ))
                }
            },
            SC::sched_yield => {
                let (time_ns, _, _, _) = rsc.args;
                SyscallResult::Switch(Ok(0), WaitFor::None)
            },
            SC::sched_sleep_ns => {
                use crate::time::SYSCLOCK;
                use core::time::Duration;
                let (time_ns, _, _, _) = rsc.args;
                let now = SYSCLOCK.now();
                SyscallResult::Switch(Ok(0), WaitFor::Time(now + Duration::from_nanos(time_ns)))
            },
            other => unimplemented!(
                "System call {:?} (0x{:02x}) not implemented yet",
                other,
                rsc.routine
            ),
        }
    } else {
        SyscallResult::Terminate(process::ProcessResult::Failed(
            process::Error::SyscallNumber(rsc.routine),
        ))
    }
}

/// Action that the interrupt handler takes
#[derive(Debug, Clone)]
pub enum SyscallResultAction {
    /// Terminate current process, and switch to the next one
    Terminate(process::ProcessResult),
    /// Continue running the current process
    Continue,
    /// Switch to the next process
    Switch(WaitFor),
}

#[must_use]
pub fn handle_syscall(
    pid: ProcessId, page_table: PhysAddr, process_stack: VirtAddr,
) -> SyscallResultAction {
    let mut sched = SCHEDULER.try_lock().unwrap();
    let process = sched.process_by_id_mut(pid).expect("Process not found");

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

        let rsc = RawSyscall {
            routine: reg_rax,
            args: (reg_rdi, reg_rsi, reg_rdx, reg_rcx),
        };
        let res = syscall(mm, process, rsc);

        // Write result register values into the process stack
        if let SyscallResult::Continue(r) | SyscallResult::Switch(r, _) = res {
            unsafe {
                match r {
                    Ok(v) => {
                        ptr::write(stack_ptr.add(0), 1); // Success
                        ptr::write(stack_ptr.add(5), v); // Value
                    },
                    Err(v) => {
                        ptr::write(stack_ptr.add(0), 0); // Error
                        ptr::write(stack_ptr.add(5), v); // Value
                    },
                }
            }
        }

        let action = match res {
            SyscallResult::Continue(_) => SyscallResultAction::Continue,
            SyscallResult::Switch(_, s) => SyscallResultAction::Switch(s),
            SyscallResult::Terminate(r) => SyscallResultAction::Terminate(r),
            SyscallResult::RepeatAfter(s) => {
                process.repeat_syscall = Some(rsc);
                SyscallResultAction::Switch(s)
            },
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

        action
    })
}
