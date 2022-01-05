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
    mem_set_size = 0x03,
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
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive, Deserialize, Serialize)]
#[allow(non_camel_case_types)]
#[repr(u64)]
pub enum SyscallErrorCode {
    unknown = 0,
    /// Empty list given, but now allowed
    empty_list_argument,
    /// System call done in nonblocking mode would block
    would_block,
    /// Trying to create a node which already exists
    fs_node_exists,
    /// Trying to create a node but parent path is blocked by a leaf
    /// If the last element is leaf, fs_node_exists is returned instead
    fs_node_path_blocked,
    /// Node is requested but does not exist
    fs_node_not_found,
    /// This operation requires a leaf node
    fs_node_not_leaf,
    /// This operation requires a non-leaf node
    fs_node_is_leaf,
    /// Invalid control function
    fs_unknown_control_function,
    /// File operation not supported, e.g. read-only files
    fs_operation_not_supported,
    /// File was destroyed while an operation was pending
    /// Normal files never do this, but processes, attachments and pipes do
    fs_file_destroyed,
    /// File is (is not | does not have an associated) process
    fs_node_not_process,
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
}
