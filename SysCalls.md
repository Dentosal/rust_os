System call reference
=====================

# List

Number | Name           | Arguments (logical)   | On success  | Short description
-------|----------------|-----------------------|-------------|-------------------
0x00   | exit           | status_code           | !           | Terminate the calling process
0x01   | get_pid        |                       | pid         | Get pid of the calling process
0x02   | debug_print    | str_len, *str_ptr*    | -           | Print a UTF-8 string to the kernel terminal
0x03   | mem_set_size   | total_bytes           | total_bytes | Set dynamic memory size, rounds up to page size
0x30   | fs_fileinfo    | **path**, *dst*       | *FileInfo*  | Get metadata about a file
0x32   | fs_open        | **path**              | fd          | Open a file from vfs
0x33   | fs_exec        | **path**              | fd          | Execute a file from vfs
0x34   | fs_attach      | **path**, is_leaf?    | fd          | Create a fs node and manage it
0x40   | fd_close       | fd                    | -           | Close fd, closing a mount unmounts
0x41   | fd_read        | fd, *buf*, count      | byte_count  | Reads `count` bytes from `fd` to `buf_ptr`
0x42   | fd_write       | fd, *buf*, count      | byte_count  | Writes `count` bytes from `buf_ptr` to `fd`
0x43   | fd_synchronize | fd                    | -           | Ensures all written data has been delivered
0x44   | fd_control     | fd, function          | -           | Send control function for a file
0x45   | fd_select      | **fds**, (timeout)    | -           | Wait until first fd is available
0x50   | sched_yield    | -                     | -           | Yield control to schedule next process
0x51   | sched_sleep_ns | ns                    | -           | Sleep specified number of nanoseconds

*Cursived* text implies that something is a pointer.
**Bold** text implies that something is a read-only slice, i.e. `len, ptr` pair.
Values like `ok?` ending with `?` represent booleans.

## Call structure

Register | Description
---------|-------------
rax      | Routine number
rdi      | Argument 1
rsi      | Argument 2
rdx      | Argument 3
rcx      | Argument 4

## Return structure

Register | Description
---------|-------------
rax      | Success? Boolean
rdi      | Return value
