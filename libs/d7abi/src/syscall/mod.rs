use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};

mod types;

pub use self::types::*;

#[derive(Debug, TryFromPrimitive)]
#[allow(non_camel_case_types)]
#[repr(u64)]
pub enum SyscallNumber {
    exit = 0x00,
    get_pid = 0x01,
    debug_print = 0x02,
    exec = 0x30,
    random = 0x40,
    sched_yield = 0x50,
    sched_sleep_ns = 0x51,
    ipc_subscribe = 0x70,
    ipc_unsubscribe = 0x71,
    ipc_publish = 0x72,
    ipc_deliver = 0x73,
    ipc_deliver_reply = 0x74,
    ipc_receive = 0x75,
    ipc_acknowledge = 0x76,
    ipc_select = 0x77,
    kernel_log_read = 0x80,
    irq_set_handler = 0x84,
    mmap_physical = 0x90,
    dma_allocate = 0x92,
    dma_free = 0x93,
    mem_alloc = 0x94,
    mem_dealloc = 0x95,
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive, Deserialize, Serialize)]
#[allow(non_camel_case_types)]
#[repr(u64)]
pub enum SyscallErrorCode {
    unknown = 0,
    /// Requested operation is not supported yet
    unsupported,
    /// Not enough memory available for requested action
    out_of_memory,
    /// Empty list given, but now allowed
    empty_list_argument,
    /// Argument is too large to process
    too_large,
    /// System call done in nonblocking mode would block
    would_block,
    /// Invalid topic or topic filter
    ipc_invalid_topic,
    /// Mutually exclusive filter is already in use
    ipc_filter_exclusion,
    /// Reliable transfer failed: no targets selected
    ipc_delivery_no_target,
    /// Reliable transfer failed: target inbox is full
    ipc_delivery_target_full,
    /// Reliable transfer failed: target negative acknowledged
    ipc_delivery_target_nack,
    /// Attempt use unsubscribed id
    ipc_unsubscribed,
    /// Attempt to acknowledge a message again
    ipc_re_acknowledge,
    /// Someone else has already connected to this pipe
    ipc_pipe_reserved,
    /// Sender side process of the pipe has been terminated
    ipc_pipe_sender_terminated,
    /// Permission error
    ipc_permission_error,
    /// Invalid UTF-8
    invalid_utf8,
    /// Invalid alignment of a pointer
    ptr_unaligned,
    /// Invalid or unsupported memory protection flags given to mmap
    mmap_invalid_protection_flags,
    /// A specific aligment or size is required, but not respected
    mmap_incorrect_alignment,
    /// Operation is not allowed
    mmap_permission_error,
}
