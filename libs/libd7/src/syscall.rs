use core::arch::asm;
use core::convert::TryFrom;
use x86_64::{PhysAddr, VirtAddr};

use d7abi::{
    ipc::{AcknowledgeId, SubscriptionId},
    process::ProcessId,
    SyscallNumber,
};

pub use d7abi::{ipc::SubscriptionFlags, MemoryProtectionFlags, SyscallErrorCode};

macro_rules! syscall {
    ($n:expr; $a0:expr, $a1:expr, $a2:expr, $a3:expr) => {
        syscall($n as u64, ($a0, $a1, $a2, $a3))
    };
    ($n:expr; $a0:expr, $a1:expr, $a2:expr) => {syscall!($n; $a0, $a1, $a2, 0)};
    ($n:expr; $a0:expr, $a1:expr) => {syscall!($n; $a0, $a1, 0, 0)};
    ($n:expr; $a0:expr) => {syscall!($n; $a0, 0, 0, 0)};
    ($n:expr) => {syscall!($n; 0, 0, 0, 0)};
}

#[must_use]
pub type SyscallResult<T> = Result<T, SyscallErrorCode>;

/// # Safety
/// Allows any unsafe system call to be called, and doesn't protect from invalid arguments.
pub unsafe fn syscall(number: u64, args: (u64, u64, u64, u64)) -> SyscallResult<u64> {
    let mut success: u64;
    let mut result: u64;

    asm!("int 0xd7",
        inout("rax") number => success,
        inout("rdi") args.0 => result,
        in("rsi") args.1,
        in("rdx") args.2,
        in("rcx") args.3,
    );

    if success == 1 {
        Ok(result)
    } else if success == 0 {
        Err(SyscallErrorCode::try_from(result)
            .unwrap_or_else(|_| panic!("System call: invalid error code {}", result)))
    } else {
        panic!("System call: invalid boolean for success {}", success);
    }
}

pub fn exit(return_code: u64) -> ! {
    unsafe {
        asm!("int 0xd7",
            in("rax") SyscallNumber::exit as u64,
            in("rdi") return_code,
            options(nomem, nostack, noreturn)
        )
    }
}

/// This system call never fails
pub fn get_pid() -> ProcessId {
    ProcessId::from_u64(unsafe { syscall!(SyscallNumber::get_pid).unwrap() })
}

/// This system call never fails
pub fn debug_print(s: &str) {
    let len = s.len() as u64;
    let slice = s.as_ptr() as u64;
    unsafe {
        syscall!(SyscallNumber::debug_print; len, slice).unwrap();
    }
}

/// Start a new process from an ELF image
pub fn exec(image: &[u8], args: &[&str]) -> SyscallResult<ProcessId> {
    let len = image.len() as u64;
    let slice = image.as_ptr() as u64;

    // TODO: maybe figure out how to do this without allocation?
    let mut args_raw: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    args_raw.extend(&(args.len() as u64).to_le_bytes());
    for arg in args {
        args_raw.extend(&(arg.len() as u64).to_le_bytes());
    }
    for arg in args {
        args_raw.extend(arg.as_bytes().iter());
    }

    unsafe {
        Ok(ProcessId::from_u64(
            syscall!(SyscallNumber::exec; len, slice, args_raw.len() as u64, args_raw.as_ptr() as u64)?,
        ))
    }
}

/// Access kernel entropy pool
pub fn random(seed: u64) -> u64 {
    unsafe { syscall!(SyscallNumber::random; seed).expect("random returned an error") }
}

/// This system call never fails, and does not return anything
pub fn sched_yield() {
    let _ = unsafe { syscall!(SyscallNumber::sched_yield) };
}

/// Max sleep time is 2**64 ns, about 584 years.
pub fn sched_sleep_ns(ns: u64) -> SyscallResult<()> {
    unsafe { syscall!(SyscallNumber::sched_sleep_ns; ns).map(|_| ()) }
}

/// Subscribes to message by a filter
pub fn ipc_subscribe(filter: &str, flags: SubscriptionFlags) -> SyscallResult<SubscriptionId> {
    let len = filter.len() as u64;
    let slice = filter.as_ptr() as u64;
    unsafe {
        Ok(SubscriptionId::from_u64(syscall!(
            SyscallNumber::ipc_subscribe;
            len, slice,
            flags.bits()
        )?))
    }
}

/// Unsubscribes from messages
pub fn ipc_unsubscribe(sub_id: SubscriptionId) -> SyscallResult<()> {
    unsafe {
        syscall!(
            SyscallNumber::ipc_unsubscribe;
            sub_id.as_u64()
        )
        .map(|_| ())
    }
}

/// Publish unreliable message (asynchronous)
pub fn ipc_publish(topic: &str, data: &[u8]) -> SyscallResult<()> {
    let len = topic.len() as u64;
    let slice = topic.as_ptr() as u64;
    unsafe {
        syscall!(
            SyscallNumber::ipc_publish;
            len, slice,
            data.len() as u64, data.as_ptr() as u64
        )
        .map(|_| ())
    }
}

/// Deliver reliable message (blocking)
pub fn ipc_deliver(topic: &str, data: &[u8]) -> SyscallResult<()> {
    let len = topic.len() as u64;
    let slice = topic.as_ptr() as u64;
    unsafe {
        syscall!(
            SyscallNumber::ipc_deliver;
            len, slice,
            data.len() as u64, data.as_ptr() as u64
        )
        .map(|_| ())
    }
}

/// Deliver a reply to a reliable message
pub fn ipc_deliver_reply(topic: &str, data: &[u8]) -> SyscallResult<()> {
    let len = topic.len() as u64;
    let slice = topic.as_ptr() as u64;
    unsafe {
        syscall!(
            SyscallNumber::ipc_deliver_reply;
            len, slice,
            data.len() as u64, data.as_ptr() as u64
        )
        .map(|_| ())
    }
}

/// Receive a message (blocking)
pub fn ipc_receive(sub_id: SubscriptionId, buf: &mut [u8]) -> SyscallResult<usize> {
    unsafe {
        syscall!(
            SyscallNumber::ipc_receive;
            sub_id.as_u64(),
            buf.len() as u64, buf.as_ptr() as u64
        )
        .map(|count| count as usize)
    }
}

/// Acknowledge a reliable message
pub fn ipc_acknowledge(
    sub_id: SubscriptionId, ack_id: AcknowledgeId, positive: bool,
) -> SyscallResult<()> {
    unsafe {
        syscall!(
            SyscallNumber::ipc_acknowledge;
            sub_id.as_u64(),
            ack_id.as_u64(),
            positive as u64
        )
        .map(|_| ())
    }
}

/// Select first available message from a list of subscriptions
pub fn ipc_select(sub_ids: &[SubscriptionId], nonblocking: bool) -> SyscallResult<usize> {
    if sub_ids.is_empty() {
        panic!("Cannot ipc_select from an empty list");
    }

    unsafe {
        Ok(syscall!(
            SyscallNumber::ipc_select;
            sub_ids.len() as u64,
            sub_ids.as_ptr() as u64,
            nonblocking as u64
        )? as usize)
    }
}

/// Read (and clear) kernel log buffer. Nonblocking.
pub fn kernel_log_read(buffer: &mut [u8]) -> SyscallResult<usize> {
    if buffer.is_empty() {
        panic!("Cannot read to an empty buffer");
    }

    unsafe {
        Ok(syscall!(
            SyscallNumber::kernel_log_read;
            buffer.len() as u64,
            buffer.as_ptr() as u64
        )? as usize)
    }
}

/// Assigns code to be ran on interrupt handler.
/// Code must be an executable sequence of instructions,
/// modifies no registers except `rax`, that will be sent
/// to the device driver when publishing the event.
///
/// # Safety
///
/// Will lead to kernel crash or silent data corruption when misused.
pub unsafe fn irq_set_handler(irq: u8, code: &mut [u8]) -> SyscallResult<()> {
    let len = code.len() as u64;
    let ptr = code.as_ptr() as u64;

    syscall!(SyscallNumber::irq_set_handler; irq as u64, len, ptr)?;
    Ok(())
}

/// Map a physical address block to this process.
///
/// # Safety
///
/// Extremely unsafe.
/// Can override process address mappings.
/// Can override kernel data.
pub unsafe fn mmap_physical(
    phys_addr: PhysAddr, virt_addr: VirtAddr, len: u64, flags: MemoryProtectionFlags,
) -> SyscallResult<*mut u8> {
    if len == 0 {
        panic!("Cannot mmap_physical an empty region");
    }

    Ok(syscall!(
        SyscallNumber::mmap_physical;
        len,
        phys_addr.as_u64(),
        virt_addr.as_u64(),
        flags.bits() as u64
    )? as *mut u8)
}

/// Allocate DMA-accessible region of the physical memory.
/// Doesn't map the memory to process
pub fn dma_allocate(len: u64) -> SyscallResult<PhysAddr> {
    if len == 0 {
        panic!("Cannot dma_allocate an empty region");
    }

    Ok(PhysAddr::new(unsafe {
        syscall!(
            SyscallNumber::dma_allocate;
            len
        )?
    }))
}

/// Must be only used with a valid address.
pub unsafe fn dma_free(phys_addr: PhysAddr, len: u64) -> SyscallResult<()> {
    if len == 0 {
        panic!("Cannot dma_free an empty region");
    }

    syscall!(
        SyscallNumber::dma_free;
        len,
        phys_addr.as_u64()
    )?;
    Ok(())
}

/// Request a virtual memory area to be backed with some physical memory
///
/// # Safety
///
/// Can be used to confuse thé memory manager, and usually
/// should not be used outside of this library.
pub unsafe fn mem_alloc(
    virt_addr: VirtAddr, len: usize, flags: MemoryProtectionFlags,
) -> SyscallResult<()> {
    if len == 0 {
        panic!("Cannot mem_alloc an empty region");
    }

    syscall!(
        SyscallNumber::mem_alloc;
        len as u64,
        virt_addr.as_u64(),
        flags.bits() as u64
    )?;
    Ok(())
}

/// Request a virtual memory area to be unmapped, and the
/// physical memory backing that region to be freed
///
/// # Safety
///
/// Can be used to confuse thé memory manager, and usually
/// should not be used outside of this library.
pub unsafe fn mem_dealloc(virt_addr: VirtAddr, len: usize) -> SyscallResult<()> {
    if len == 0 {
        panic!("Cannot mem_alloc an empty region");
    }

    syscall!(
        SyscallNumber::mem_dealloc;
        len as u64,
        virt_addr.as_u64()
    )?;
    Ok(())
}
