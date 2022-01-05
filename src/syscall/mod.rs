use alloc::vec::Vec;
use core::convert::{TryFrom, TryInto};
use core::mem;
use core::ptr;
use core::time::Duration;
use hashbrown::HashSet;
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{PhysAddr, VirtAddr};

use d7abi::ipc::SubscriptionFlags;
use d7abi::SyscallErrorCode as ErrorCode;

use crate::ipc;
use crate::memory::prelude::*;
use crate::memory::{self, MemoryController};
use crate::multitasking::{process, Process, ProcessId, Scheduler, WaitFor, SCHEDULER};
use crate::time::BSPInstant;

/// Separate module to get distinct logging path
#[allow(non_snake_case)]
mod PROCESS_OUTPUT {
    use d7abi::process::ProcessId;

    pub fn print(pid: ProcessId, string: &str) {
        log::info!("[pid={:8}] {}", pid.as_u64(), string);
        // TODO: Sometimes terminal is locked when this happens
        // if crate::syslog::LEVEL_SCREEN < log::Level::Debug {
        //     rprintln!("[pid={}] {}", pid.as_u64(), string);
        // }
    }
}

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

macro_rules! try_str {
    ($slice:expr) => {{
        // Sanity check
        assert!($slice.len() < 10000, "String length sanity check"); // TODO: client error
        match ::core::str::from_utf8($slice) {
            Ok(value) => value,
            Err(err) => {
                return SyscallResult::Continue(Err(ErrorCode::invalid_utf8.into()));
            },
        }
    }};
}

macro_rules! try_ipc {
    ($ipc_result:expr) => {
        match $ipc_result {
            Ok(value) => value,
            Err(error) => {
                return SyscallResult::Continue(Err(error.into()));
            },
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSyscall {
    pub routine: u64,
    args: (u64, u64, u64, u64),
}

fn syscall(
    m: &mut MemoryController, sched: &mut Scheduler, pid: ProcessId, rsc: RawSyscall,
) -> SyscallResult {
    use d7abi::SyscallNumber as SC;

    let process = sched.process_by_id_mut(pid).expect("Process not found");
    let pid = process.id();

    if let Ok(sc) = SC::try_from(rsc.routine) {
        match sc {
            SC::exit => SyscallResult::Terminate(process::ProcessResult::Completed(rsc.args.0)),
            SC::get_pid => SyscallResult::Continue(Ok(pid.as_u64())),
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
                let string = try_str!(slice);

                PROCESS_OUTPUT::print(pid, string);

                unsafe { m.unmap_area(area) };
                m.free_virtual_area(area);
                SyscallResult::Continue(Ok(0))
            },
            SC::mem_set_size => {
                let (size_bytes, _, _, _) = rsc.args;
                match m.process_set_dynamic_memory(process, size_bytes) {
                    Some(total_bytes) => SyscallResult::Continue(Ok(total_bytes)),
                    _ => unimplemented!("OutOfMemory case not implmented yet"),
                }
            },
            SC::exec => {
                let (image_len, image_ptr, _, _) = rsc.args;
                let image_ptr = VirtAddr::new(image_ptr);
                if let Some((area, slice)) =
                    unsafe { m.process_slice(process, image_len, image_ptr) }
                {
                    log::debug!("[pid={:8}] exec len={:?}", pid, slice.len());

                    let elfimage = crate::multitasking::process::load_elf(m, slice);
                    let pid = sched.spawn(m, elfimage);

                    unsafe { m.unmap_area(area) };
                    m.free_virtual_area(area);

                    SyscallResult::Continue(Ok(unsafe { pid.as_u64() }))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(image_ptr),
                    ))
                }
            },
            SC::random => {
                let (entropy, _, _, _) = rsc.args;
                crate::random::insert_entropy(entropy);
                SyscallResult::Switch(Ok(crate::random::read()), WaitFor::None)
            },
            SC::sched_yield => {
                let (_, _, _, _) = rsc.args;
                SyscallResult::Switch(Ok(0), WaitFor::None)
            },
            SC::sched_sleep_ns => {
                let (time_ns, _, _, _) = rsc.args;
                if crate::smp::is_bsp() {
                    SyscallResult::Switch(Ok(0), WaitFor::Time(BSPInstant::now().add_ns(time_ns)))
                } else {
                    todo!(); // If core != BSP, push into a set-to-sleep queue
                }
            },
            SC::ipc_subscribe => {
                let (filter_len, filter_ptr, flags, _) = rsc.args;
                let Some(flags) = SubscriptionFlags::from_bits(flags) else {
                    return SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::SyscallArgument,
                    ));
                };

                let filter_ptr = VirtAddr::new(filter_ptr);
                if let Some((area, slice)) =
                    unsafe { m.process_slice(process, filter_len, filter_ptr) }
                {
                    let filter_str = try_str!(slice);
                    let filter = try_ipc!(ipc::TopicFilter::try_new(
                        filter_str,
                        !flags.contains(SubscriptionFlags::PREFIX)
                    ));

                    log::trace!("[pid={:8}] ipc_subscribe {:?} {:?}", pid, filter, flags);

                    let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");
                    let sub_id = try_ipc!(ipc_manager.subscribe(
                        pid,
                        filter,
                        flags.contains(SubscriptionFlags::RELIABLE),
                        flags.contains(SubscriptionFlags::PIPE),
                    ));

                    unsafe { m.unmap_area(area) };
                    m.free_virtual_area(area);
                    SyscallResult::Continue(Ok(sub_id.as_u64()))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(filter_ptr),
                    ))
                }
            },
            SC::ipc_unsubscribe => {
                let (sub_id, _, _, _) = rsc.args;
                let sub_id = ipc::SubscriptionId::from_u64(sub_id);

                log::trace!("[pid={:8}] ipc_unsubscribe {:?}", pid, sub_id);

                let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");
                try_ipc!(ipc_manager.unsubscribe(pid, sub_id).consume_events(sched));

                SyscallResult::Continue(Ok(0))
            },
            SC::ipc_publish => {
                let (topic_len, topic_ptr, data_len, data_ptr) = rsc.args;
                let topic_ptr = VirtAddr::new(topic_ptr);
                let data_ptr = VirtAddr::new(data_ptr);
                if let Some((topic_area, topic_slice)) =
                    unsafe { m.process_slice(process, topic_len, topic_ptr) }
                {
                    let result = if let Some((data_area, data_slice)) =
                        unsafe { m.process_slice(process, data_len, data_ptr) }
                    {
                        let topic_str = try_str!(topic_slice);
                        let topic = try_ipc!(ipc::Topic::try_new(topic_str));

                        log::trace!(
                            "[pid={:8}] ipc_publish topic={:?} len={:?}",
                            pid,
                            topic,
                            data_len
                        );

                        let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");
                        try_ipc!(ipc_manager.publish(topic, data_slice).consume_events(sched));

                        unsafe { m.unmap_area(data_area) };
                        m.free_virtual_area(data_area);
                        SyscallResult::Continue(Ok(0))
                    } else {
                        SyscallResult::Terminate(process::ProcessResult::Failed(
                            process::Error::Pointer(data_ptr),
                        ))
                    };
                    unsafe { m.unmap_area(topic_area) };
                    m.free_virtual_area(topic_area);
                    result
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(topic_ptr),
                    ))
                }
            },
            SC::ipc_deliver => {
                let (topic_len, topic_ptr, data_len, data_ptr) = rsc.args;
                let topic_ptr = VirtAddr::new(topic_ptr);
                let data_ptr = VirtAddr::new(data_ptr);

                let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");

                // Delivery complete
                if ipc_manager.delivery_complete(pid) {
                    log::trace!("[pid={:8}] ipc_deliver complete", pid);

                    try_ipc!(ipc_manager.after_delivery(pid).consume_events(sched));
                    return SyscallResult::Continue(Ok(0));
                }

                if let Some((topic_area, topic_slice)) =
                    unsafe { m.process_slice(process, topic_len, topic_ptr) }
                {
                    let result = if let Some((data_area, data_slice)) =
                        unsafe { m.process_slice(process, data_len, data_ptr) }
                    {
                        let topic_str = try_str!(topic_slice);
                        let topic = try_ipc!(ipc::Topic::try_new(topic_str));
                        log::trace!(
                            "[pid={:8}] ipc_deliver topic={:?} len={:?}",
                            pid,
                            topic,
                            data_len
                        );

                        let deliver = try_ipc!(
                            ipc_manager
                                .deliver(pid, topic, data_slice)
                                .consume_events(sched)
                        );

                        unsafe { m.unmap_area(data_area) };
                        m.free_virtual_area(data_area);

                        match deliver {
                            ipc::Deliver::Process(event) => {
                                SyscallResult::RepeatAfter(WaitFor::Event(event))
                            },
                            ipc::Deliver::Kernel => SyscallResult::Continue(Ok(0)),
                        }
                    } else {
                        SyscallResult::Terminate(process::ProcessResult::Failed(
                            process::Error::Pointer(data_ptr),
                        ))
                    };
                    unsafe { m.unmap_area(topic_area) };
                    m.free_virtual_area(topic_area);
                    result
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(topic_ptr),
                    ))
                }
            },
            SC::ipc_deliver_reply => {
                let (topic_len, topic_ptr, data_len, data_ptr) = rsc.args;
                let topic_ptr = VirtAddr::new(topic_ptr);
                let data_ptr = VirtAddr::new(data_ptr);

                let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");

                if let Some((topic_area, topic_slice)) =
                    unsafe { m.process_slice(process, topic_len, topic_ptr) }
                {
                    let result = if let Some((data_area, data_slice)) =
                        unsafe { m.process_slice(process, data_len, data_ptr) }
                    {
                        let topic_str = try_str!(topic_slice);
                        let topic = try_ipc!(ipc::Topic::try_new(topic_str));
                        log::trace!(
                            "[pid={:8}] ipc_deliver_reply topic={:?} len={:?}",
                            pid,
                            topic,
                            data_len
                        );

                        let result = ipc_manager
                            .deliver_reply(pid, topic, data_slice)
                            .consume_events(sched);

                        unsafe { m.unmap_area(data_area) };
                        m.free_virtual_area(data_area);

                        try_ipc!(result);
                        SyscallResult::Continue(Ok(0))
                    } else {
                        SyscallResult::Terminate(process::ProcessResult::Failed(
                            process::Error::Pointer(data_ptr),
                        ))
                    };
                    unsafe { m.unmap_area(topic_area) };
                    m.free_virtual_area(topic_area);
                    result
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(topic_ptr),
                    ))
                }
            },
            SC::ipc_receive => {
                let (sub_id, buf_len, buf_ptr, _) = rsc.args;
                let sub_id = ipc::SubscriptionId::from_u64(sub_id);
                let buf_ptr = VirtAddr::new(buf_ptr);
                if let Some((area, slice)) =
                    unsafe { m.process_slice_mut(process, buf_len, buf_ptr) }
                {
                    log::trace!(
                        "[pid={:8}] ipc_receive sub={:?} len={:?}",
                        pid,
                        sub_id,
                        buf_len
                    );

                    let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");
                    let message_or_event =
                        try_ipc!(ipc_manager.receive(pid, sub_id).consume_events(sched));

                    let msg = match message_or_event {
                        Ok(msg) => msg,
                        Err(event) => {
                            return SyscallResult::RepeatAfter(WaitFor::Event(event));
                        },
                    };

                    let ser_msg = pinecone::to_vec(&msg).unwrap();

                    if ser_msg.len() > slice.len() {
                        panic!(
                            "Buffer too small msg_len={} buf_len={}",
                            ser_msg.len(),
                            slice.len()
                        ); // TODO: client error
                    }

                    slice[..ser_msg.len()].copy_from_slice(&ser_msg);

                    unsafe { m.unmap_area(area) };
                    m.free_virtual_area(area);
                    SyscallResult::Continue(Ok(ser_msg.len() as u64))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(buf_ptr),
                    ))
                }
            },
            SC::ipc_acknowledge => {
                let (sub_id, ack_id, positive, _) = rsc.args;
                let sub_id = ipc::SubscriptionId::from_u64(sub_id);
                let ack_id = ipc::AcknowledgeId::from_u64(ack_id);
                let positive = positive != 0;

                log::trace!(
                    "[pid={:8}] ipc_acknowledge sub={:?} ack_id={:?} positive={:?}",
                    pid,
                    sub_id,
                    ack_id,
                    positive
                );

                let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");
                try_ipc!(
                    ipc_manager
                        .acknowledge(sub_id, ack_id, positive)
                        .consume_events(sched)
                );
                SyscallResult::Continue(Ok(0))
            },
            SC::ipc_select => {
                let (subs_len, subs, nonblocking, _) = rsc.args;

                if subs_len == 0 {
                    return SyscallResult::Continue(Err(ErrorCode::empty_list_argument.into()));
                }

                let subs = VirtAddr::new(subs);
                let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");
                let size = mem::size_of::<ipc::SubscriptionId>() as u64;
                let blocking = nonblocking == 0;

                log::trace!("ipc_select n={} blocking={}", subs_len, blocking);

                if let Some((area, subs_slice)) =
                    unsafe { m.process_slice(process, subs_len * size, subs) }
                {
                    let mut conditions = Vec::new();
                    for sub_bytes in subs_slice.chunks_exact(8) {
                        let sub_id = ipc::SubscriptionId::from_u64(u64::from_le_bytes(
                            sub_bytes.try_into().unwrap(),
                        ));
                        let condition = ipc_manager.waiting_for(sub_id);
                        log::trace!("* {:?} condition = {:?}", sub_id, condition);

                        if condition == WaitFor::None {
                            unsafe { m.unmap_area(area) };
                            return SyscallResult::Continue(Ok(sub_id.as_u64()));
                        }

                        conditions.push(condition);
                    }

                    unsafe { m.unmap_area(area) };
                    m.free_virtual_area(area);

                    if !blocking {
                        return SyscallResult::Continue(Err(ErrorCode::would_block.into()));
                    }

                    SyscallResult::RepeatAfter(WaitFor::FirstOf(conditions))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(subs),
                    ))
                }
            },
            SC::kernel_log_read => {
                let (buf_len, buf_ptr, _, _) = rsc.args;
                let buf_ptr = VirtAddr::new(buf_ptr);
                if let Some((area, slice)) =
                    unsafe { m.process_slice_mut(process, buf_len, buf_ptr) }
                {
                    let count = crate::syslog::syscall_read(slice);
                    unsafe { m.unmap_area(area) };
                    m.free_virtual_area(area);
                    SyscallResult::Continue(Ok(count as u64))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(buf_ptr),
                    ))
                }
            },
            SC::irq_set_handler => {
                let (ird, image_len, image_ptr, _) = rsc.args;
                todo!();
                // let image_ptr = VirtAddr::new(image_ptr);
                // if let Some((area, slice)) =
                //     unsafe { m.process_slice(process, image_len, image_ptr) }
                // {
                //     log::debug!("[pid={:8}] exec len={:?}", pid, slice.len());

                //     let elfimage = crate::multitasking::process::load_elf(m, slice);
                //     let pid = sched.spawn(m, elfimage);

                //     unsafe { m.unmap_area(area) };
                //     m.free_virtual_area(area);

                //     SyscallResult::Continue(Ok(unsafe { pid.as_u64() }))
                // } else {
                //     SyscallResult::Terminate(process::ProcessResult::Failed(
                //         process::Error::Pointer(image_ptr),
                //     ))
                // }
            },
            SC::mmap_physical => {
                use d7abi::MemoryProtectionFlags as PFlags;
                use x86_64::structures::paging::page_table::PageTableFlags;

                let (len, phys_addr, virt_addr, flags) = rsc.args;
                let phys_addr = PhysAddr::new(phys_addr);
                let virt_addr = VirtAddr::new(virt_addr);

                // Read flags
                let writable = if let Some(flags) = PFlags::from_bits(flags as u8) {
                    if flags == PFlags::READ {
                        false
                    } else if flags == (PFlags::READ | PFlags::WRITE) {
                        true
                    } else {
                        return SyscallResult::Continue(Err(
                            ErrorCode::mmap_invalid_protection_flags.into(),
                        ));
                    }
                } else {
                    return SyscallResult::Continue(Err(
                        ErrorCode::mmap_invalid_protection_flags.into()
                    ));
                };

                log::debug!(
                    "[pid={:8}] mmap_physical {:?} -> {:?} len={:#x} writable={}",
                    pid,
                    phys_addr,
                    virt_addr,
                    len,
                    writable
                );

                let flags = if writable {
                    PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE | PageTableFlags::WRITABLE
                } else {
                    PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE
                };

                // Check pointers for page-alignment
                if !virt_addr.is_aligned(PAGE_SIZE_BYTES) {
                    log::warn!("mmap_phyiscal: virt_addr is not page-aligned");
                    return SyscallResult::Continue(Err(ErrorCode::ptr_unaligned.into()));
                }
                if !phys_addr.is_aligned(PAGE_SIZE_BYTES) {
                    log::warn!("mmap_phyiscal: phys_addr is not page-aligned");
                    return SyscallResult::Continue(Err(ErrorCode::ptr_unaligned.into()));
                }

                let frames = PhysFrameRangeInclusive {
                    start: PhysFrame::containing_address(phys_addr),
                    end: PhysFrame::containing_address(phys_addr + len),
                };

                let process = sched.process_by_id_mut(pid).unwrap();
                unsafe {
                    process.modify_tables(m, |pt, curr_addr| {
                        for (i, frame) in frames.enumerate() {
                            pt.map_to(
                                curr_addr,
                                Page::from_start_address(virt_addr + (i as u64) * PAGE_SIZE_BYTES)
                                    .unwrap(),
                                frame,
                                flags,
                            )
                            .ignore();
                        }
                    });
                }

                SyscallResult::Continue(Ok(0))
            },
            SC::dma_allocate => {
                let (len, _, _, _) = rsc.args;
                assert!(len != 0); // TODO: client error
                log::debug!("[pid={:8}] dma_allocate len={}", pid, len);
                let region = m.dma_allocator.allocate(len as usize);
                SyscallResult::Continue(Ok(region.start.as_u64()))
            },
            SC::dma_free => {
                log::warn!("Ignoring syscall dma_free");
                // TODO
                SyscallResult::Continue(Ok(0))
            },
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
    if !crate::smp::is_bsp() {
        todo!("Cannot do syscalls with non-BSP cores yet");
    };

    // Map process stack
    memory::configure(|mm| {
        let mut sched = SCHEDULER
            .try_lock()
            .expect("SCHEDULER LOCKED at start of handle_syscall");

        let stack_area = mm.alloc_virtual_area(PROCESS_STACK_SIZE_PAGES);
        let process = sched.process_by_id(pid).expect("Process not found");

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
        log::trace!(
            "[pid={:8}] <= {:?} ",
            pid,
            d7abi::SyscallNumber::try_from(rsc.routine).ok()
        );
        let res = syscall(mm, &mut sched, pid, rsc);
        log::trace!("[pid={:8}] => {:?} ", pid, res);

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

        let process = sched.process_by_id_mut(pid).expect("Process not found");
        process.repeat_syscall = false;
        let action = match res {
            SyscallResult::Continue(_) => SyscallResultAction::Continue,
            SyscallResult::Switch(_, s) => SyscallResultAction::Switch(s),
            SyscallResult::Terminate(r) => SyscallResultAction::Terminate(r),
            SyscallResult::RepeatAfter(s) => {
                process.repeat_syscall = true;
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
