use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, TryFromPrimitive)]
#[allow(non_camel_case_types)]
#[repr(u64)]
pub enum SyscallNumber {
    exit = 0x00,
    get_pid = 0x01,
    debug_print = 0x02,
    mem_set_size = 0x03,
    fs_open = 0x30,
    fs_exec = 0x31,
    fs_attach = 0x32,
    fs_fileinfo = 0x33,
    fd_close = 0x40,
    fd_read = 0x41,
    fd_write = 0x42,
    fd_synchronize = 0x43,
    fd_control = 0x44,
    fd_select = 0x45,
    sched_yield = 0x50,
    sched_sleep_ns = 0x51,
}

#[derive(Debug, TryFromPrimitive, IntoPrimitive)]
#[allow(non_camel_case_types)]
#[repr(u64)]
pub enum SyscallErrorCode {
    unknown = 0,
    /// Empty list given, but now allowed
    empty_list_argument,
    /// Trying to create a node which already exists
    fs_node_exists,
    /// Node is requested but does not exist
    fs_node_not_found,
    /// This operation requires a leaf node
    fs_node_not_leaf,
    /// This operation requires a non-leaf node
    fs_node_is_leaf,
    /// Invalid control function
    fs_unknown_control_function,
    /// File does not support writing
    fs_readonly,
    /// File was destroyed while an operation was pending
    /// Normal files never do this, but processes, attachments and pipes do
    fs_file_destroyed,
    /// Invalid UTF-8
    invalid_utf8,
}
