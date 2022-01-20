use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use core::convert::{TryFrom, TryInto};
use core::time::Duration;
use core::{fmt, mem, ptr};
use hashbrown::HashSet;
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{PhysAddr, VirtAddr};

use d7abi::ipc::SubscriptionFlags;
use d7abi::SyscallErrorCode as ErrorCode;

use crate::ipc;
use crate::memory::phys::OutOfMemory;
use crate::memory::{self, phys_to_virt, prelude::*};
use crate::multitasking::{process, Process, ProcessId, Scheduler, WaitFor, SCHEDULER};
use crate::time::BSPInstant;

/// Separate module to get distinct logging path
#[allow(non_snake_case)]
mod PROCESS_OUTPUT {
    use d7abi::process::ProcessId;

    pub fn print(pid: ProcessId, string: &str) {
        log::info!("[pid={:2}] {}", pid.as_u64(), string);
    }
}

#[derive(Clone)]
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

fn _fmt_return_code(r: Result<u64, u64>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match r {
        Err(ec) => match ErrorCode::try_from(ec) {
            Ok(ec) => write!(f, "Err({:?})", ec),
            Err(()) => write!(f, "Err(invalid-error-code)"),
        },
        Ok(v) => write!(f, "Ok({:?})", v),
    }
}

impl fmt::Debug for SyscallResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyscallResult::Continue(r) => {
                write!(f, "Continue(")?;
                _fmt_return_code(*r, f)?;
                write!(f, ")")
            },
            SyscallResult::Switch(r, w) => {
                write!(f, "Switch(")?;
                _fmt_return_code(*r, f)?;
                write!(f, ", {:?})", w)
            },
            SyscallResult::RepeatAfter(w) => write!(f, "RepeatAfter({:?})", w),
            SyscallResult::Terminate(t) => write!(f, "Terminate({:?})", t),
        }
    }
}

macro_rules! try_len {
    ($len:expr) => {{
        if $len > 0x400000000 {
            // Arbitrary limit of 16 GiB
            return SyscallResult::Continue(Err(ErrorCode::too_large.into()));
        } else {
            $len as usize
        }
    }};
}

macro_rules! try_str {
    ($slice:expr) => {{
        // Sanity check
        assert!($slice.len() < 10000, "String length sanity check"); // TODO: client error
        match ::core::str::from_utf8($slice) {
            Ok(value) => value,
            Err(_) => {
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

fn syscall(sched: &mut Scheduler, process: &mut Process, rsc: RawSyscall) -> SyscallResult {
    use d7abi::SyscallNumber as SC;

    let pid = process.id();

    if let Ok(sc) = SC::try_from(rsc.routine) {
        match sc {
            SC::exit => SyscallResult::Terminate(process::ProcessResult::Completed(rsc.args.0)),
            SC::get_pid => SyscallResult::Continue(Ok(pid.as_u64())),
            SC::debug_print => {
                let (str_len, str_ptr, _, _) = rsc.args;

                let str_ptr = VirtAddr::new(str_ptr);
                let str_len = try_len!(str_len);
                let (_area, slice) = unsafe {
                    match process.memory_slice(str_ptr, str_len) {
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

                SyscallResult::Continue(Ok(0))
            },
            SC::exec => {
                let (image_len, image_ptr, args_size, args_ptr) = rsc.args;
                let image_len = try_len!(image_len);
                let args_size = try_len!(args_size);

                // TODO: maybe the system call should take args in some other format?
                let mut args: Vec<String> = Vec::new();
                let args_ptr = VirtAddr::new(args_ptr);
                if let Some((_area, slice)) = unsafe { process.memory_slice(args_ptr, args_size) } {
                    let mut buf = [0u8; 8];
                    buf.copy_from_slice(&slice[..8]);
                    let argc = u64::from_le_bytes(buf) as usize;

                    let arg_len = |i: usize| -> usize {
                        let mut buf = [0u8; 8];
                        buf.copy_from_slice(&slice[(1 + i) * 8..][..8]);
                        u64::from_le_bytes(buf) as usize
                    };

                    let mut cursor = (1 + argc) * 8;
                    for i in 0..argc {
                        let len = arg_len(i);
                        args.push(try_str!(&slice[cursor..][..len]).to_owned());
                        cursor += len;
                    }
                } else {
                    return SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(args_ptr),
                    ));
                }

                let image_ptr = VirtAddr::new(image_ptr);
                if let Some((_area, slice)) = unsafe { process.memory_slice(image_ptr, image_len) }
                {
                    log::debug!("[pid={:2}] exec len={:?} args={:?}", pid, slice.len(), args);

                    let Ok(elfimage) = crate::multitasking::load_elf(slice) else {
                        return SyscallResult::Continue(Err(ErrorCode::out_of_memory.into()));
                    };

                    log::debug!("[pid={:2}] exec elf ok", pid);

                    match sched.spawn(args.as_slice(), elfimage) {
                        Ok(pid) => SyscallResult::Continue(Ok(unsafe { pid.as_u64() })),
                        Err(OutOfMemory) => {
                            SyscallResult::Continue(Err(ErrorCode::out_of_memory.into()))
                        },
                    }
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
                log::trace!(
                    "[pid={:2}] sleep_ns {} ({} millis)",
                    pid,
                    time_ns,
                    time_ns / 1_000_000
                );
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

                let filter_len = try_len!(filter_len);
                let filter_ptr = VirtAddr::new(filter_ptr);
                if let Some((_area, slice)) =
                    unsafe { process.memory_slice(filter_ptr, filter_len) }
                {
                    let filter_str = try_str!(slice);
                    let filter = try_ipc!(ipc::TopicFilter::try_new(
                        filter_str,
                        !flags.contains(SubscriptionFlags::PREFIX)
                    ));

                    log::debug!("[pid={:2}] ipc_subscribe {:?} {:?}", pid, filter, flags);

                    let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");
                    let sub_id = try_ipc!(ipc_manager.subscribe(
                        pid,
                        filter,
                        flags.contains(SubscriptionFlags::RELIABLE),
                        flags.contains(SubscriptionFlags::PIPE),
                    ));

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

                log::debug!("[pid={:2}] ipc_unsubscribe {:?}", pid, sub_id);

                let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");
                try_ipc!(ipc_manager.unsubscribe(pid, sub_id).consume_events(sched));

                SyscallResult::Continue(Ok(0))
            },
            SC::ipc_publish => {
                let (topic_len, topic_ptr, data_len, data_ptr) = rsc.args;
                let topic_len = try_len!(topic_len);
                let topic_ptr = VirtAddr::new(topic_ptr);
                let data_len = try_len!(data_len);
                let data_ptr = VirtAddr::new(data_ptr);

                let topic = if let Some((_area, topic_slice)) =
                    unsafe { process.memory_slice(topic_ptr, topic_len) }
                {
                    let topic_str = try_str!(topic_slice);
                    try_ipc!(ipc::Topic::try_new(topic_str))
                } else {
                    return SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(topic_ptr),
                    ));
                };

                log::trace!(
                    "[pid={:2}] ipc_publish topic={:?} len={:?}",
                    pid,
                    topic,
                    data_len
                );

                if let Some((_area, data_slice)) =
                    unsafe { process.memory_slice(data_ptr, data_len) }
                {
                    let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");
                    try_ipc!(ipc_manager.publish(topic, data_slice).consume_events(sched));

                    SyscallResult::Continue(Ok(0))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(data_ptr),
                    ))
                }
            },
            SC::ipc_deliver => {
                let (topic_len, topic_ptr, data_len, data_ptr) = rsc.args;
                let topic_len = try_len!(topic_len);
                let topic_ptr = VirtAddr::new(topic_ptr);
                let data_len = try_len!(data_len);
                let data_ptr = VirtAddr::new(data_ptr);

                let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");

                // Delivery complete
                if ipc_manager.delivery_complete(pid) {
                    log::trace!("[pid={:2}] ipc_deliver complete", pid);

                    try_ipc!(ipc_manager.after_delivery(pid).consume_events(sched));
                    return SyscallResult::Continue(Ok(0));
                }

                let topic = if let Some((_area, topic_slice)) =
                    unsafe { process.memory_slice(topic_ptr, topic_len) }
                {
                    let topic_str = try_str!(topic_slice);
                    try_ipc!(ipc::Topic::try_new(topic_str))
                } else {
                    return SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(topic_ptr),
                    ));
                };

                log::trace!(
                    "[pid={:2}] ipc_deliver topic={:?} len={:?}",
                    pid,
                    topic,
                    data_len
                );

                if let Some((_area, data_slice)) =
                    unsafe { process.memory_slice(data_ptr, data_len) }
                {
                    let deliver = try_ipc!(
                        ipc_manager
                            .deliver(pid, topic, data_slice)
                            .consume_events(sched)
                    );

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
                }
            },
            SC::ipc_deliver_reply => {
                let (topic_len, topic_ptr, data_len, data_ptr) = rsc.args;
                let topic_len = try_len!(topic_len);
                let topic_ptr = VirtAddr::new(topic_ptr);
                let data_len = try_len!(data_len);
                let data_ptr = VirtAddr::new(data_ptr);

                let mut ipc_manager = ipc::IPC.try_lock().expect("IPC LOCKED");

                let topic = if let Some((_area, topic_slice)) =
                    unsafe { process.memory_slice(topic_ptr, topic_len) }
                {
                    let topic_str = try_str!(topic_slice);
                    try_ipc!(ipc::Topic::try_new(topic_str))
                } else {
                    return SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(topic_ptr),
                    ));
                };

                log::trace!(
                    "[pid={:2}] ipc_deliver_reply topic={:?} len={:?}",
                    pid,
                    topic,
                    data_len
                );

                if let Some((_area, data_slice)) =
                    unsafe { process.memory_slice(data_ptr, data_len) }
                {
                    let result = ipc_manager
                        .deliver_reply(pid, topic, data_slice)
                        .consume_events(sched);

                    try_ipc!(result);
                    SyscallResult::Continue(Ok(0))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(data_ptr),
                    ))
                }
            },
            SC::ipc_receive => {
                let (sub_id, buf_len, buf_ptr, _) = rsc.args;
                let sub_id = ipc::SubscriptionId::from_u64(sub_id);
                let buf_len = try_len!(buf_len);
                let buf_ptr = VirtAddr::new(buf_ptr);
                if let Some((_area, slice)) = unsafe { process.memory_slice_mut(buf_ptr, buf_len) }
                {
                    log::trace!(
                        "[pid={:2}] ipc_receive sub={:?} len={:?}",
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
                    "[pid={:2}] ipc_acknowledge sub={:?} ack_id={:?} positive={:?}",
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

                if let Some((_area, subs_slice)) =
                    unsafe { process.memory_slice(subs, try_len!(subs_len * size)) }
                {
                    let mut conditions = Vec::new();
                    for (index, sub_bytes) in subs_slice.chunks_exact(8).enumerate() {
                        let sub_id = ipc::SubscriptionId::from_u64(u64::from_le_bytes(
                            sub_bytes.try_into().unwrap(),
                        ));
                        let condition = ipc_manager.waiting_for(sub_id);
                        log::trace!("* {:?} condition = {:?}", sub_id, condition);

                        if condition == WaitFor::None {
                            return SyscallResult::Continue(Ok(index as u64));
                        }

                        conditions.push(condition);
                    }

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
                let buf_len = try_len!(buf_len);
                let buf_ptr = VirtAddr::new(buf_ptr);
                if let Some((_area, slice)) = unsafe { process.memory_slice_mut(buf_ptr, buf_len) }
                {
                    let count = crate::syslog::syscall_read(slice);
                    SyscallResult::Continue(Ok(count as u64))
                } else {
                    SyscallResult::Terminate(process::ProcessResult::Failed(
                        process::Error::Pointer(buf_ptr),
                    ))
                }
            },
            SC::irq_set_handler => {
                let (_ird, _image_len, _image_ptr, _) = rsc.args;
                todo!();
                // let image_len = try_len!(image_len);
                // let image_ptr = VirtAddr::new(image_ptr);
                // if let Some((_area, slice)) =
                //     unsafe { process.memory_slice( image_len, image_ptr) }
                // {
                //     log::debug!("[pid={:2}] exec len={:?}", pid, slice.len());

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
                    "[pid={:2}] mmap_physical {:?} -> {:?} len={:#x} writable={}",
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

                unsafe {
                    let proc_pt = phys_to_virt(process.page_table.phys_addr);

                    for (i, frame) in frames.enumerate() {
                        process
                            .page_table
                            .map_to(
                                proc_pt,
                                Page::from_start_address(virt_addr + (i as u64) * PAGE_SIZE_BYTES)
                                    .unwrap(),
                                frame,
                                flags,
                            )
                            .ignore();
                    }
                }

                SyscallResult::Continue(Ok(0))
            },
            SC::dma_allocate => {
                let (len, _, _, _) = rsc.args;
                assert!(len != 0); // TODO: client error
                log::debug!("[pid={:2}] dma_allocate len={}", pid, len);
                let mut dma_a = memory::dma_allocator::DMA_ALLOCATOR.lock();
                let region = dma_a.allocate(len as usize);
                SyscallResult::Continue(Ok(region.start.as_u64()))
            },
            SC::dma_free => {
                log::warn!("Ignoring syscall dma_free");
                // TODO
                SyscallResult::Continue(Ok(0))
            },
            SC::mem_alloc => {
                use d7abi::MemoryProtectionFlags as PFlags;

                let (area_len, area_ptr, flags, _) = rsc.args;
                let area_len = area_len as usize;
                let area_ptr = VirtAddr::new(area_ptr);

                // Read flags
                let Some(flags) = PFlags::from_bits(flags as u8) else {
                    return SyscallResult::Continue(Err(
                        ErrorCode::mmap_invalid_protection_flags.into()
                    ));
                };

                log::debug!(
                    "[pid={:2}] mem_alloc ptr={:p} len={:x} flags={:?}",
                    pid,
                    area_ptr,
                    area_len,
                    flags
                );

                match process.memory_alloc(area_ptr, area_len, flags) {
                    Ok(()) => SyscallResult::Continue(Ok(0)),
                    Err(code) => SyscallResult::Continue(Err(code.into())),
                }
            },
            SC::mem_dealloc => {
                let (area_len, area_ptr, _, _) = rsc.args;
                let area_len = area_len as usize;
                let area_ptr = VirtAddr::new(area_ptr);

                log::debug!(
                    "[pid={:2}] mem_dealloc ptr={:p} len={:x}",
                    pid,
                    area_ptr,
                    area_len
                );

                match process.memory_dealloc(area_ptr, area_len) {
                    Ok(()) => SyscallResult::Continue(Ok(0)),
                    Err(code) => SyscallResult::Continue(Err(code.into())),
                }
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
pub fn handle_syscall(pid: ProcessId) -> SyscallResultAction {
    if !crate::smp::is_bsp() {
        todo!("Cannot do syscalls with non-BSP cores yet");
    };

    let mut sched = SCHEDULER
        .try_lock()
        .expect("SCHEDULER LOCKED at start of handle_syscall");

    // Take process from the scheduler
    // Safety: we must give this back before returning
    let mut process = unsafe { sched.take_process_by_id(pid).expect("Process not found") };

    // Read process stack
    let reg_rax: u64 = unsafe { process.read_stack_u64(0) };
    let reg_rdi: u64 = unsafe { process.read_stack_u64(5) };
    let reg_rsi: u64 = unsafe { process.read_stack_u64(4) };
    let reg_rdx: u64 = unsafe { process.read_stack_u64(3) };
    let reg_rcx: u64 = unsafe { process.read_stack_u64(2) };

    let rsc = RawSyscall {
        routine: reg_rax,
        args: (reg_rdi, reg_rsi, reg_rdx, reg_rcx),
    };
    log::trace!(
        "[pid={:2}] <= {:?} ",
        pid,
        d7abi::SyscallNumber::try_from(rsc.routine).ok()
    );
    let res = syscall(&mut sched, &mut process, rsc);
    log::trace!("[pid={:2}] => {:?} ", pid, res);

    // Write result register values into the process stack
    if let SyscallResult::Continue(r) | SyscallResult::Switch(r, _) = res {
        unsafe {
            match r {
                Ok(v) => {
                    process.write_stack_u64(0, 1); // Success
                    process.write_stack_u64(5, v); // Value
                },
                Err(v) => {
                    process.write_stack_u64(0, 0); // Error
                    process.write_stack_u64(5, v); // Value
                },
            }
        }
    }

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

    // Give the process back to the scheduler
    // Safety: we got this from the scheduler, as required
    unsafe {
        sched.give_back_process(process);
    }

    action
}
