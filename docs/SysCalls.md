System call reference
=====================

# List

Number | Name           | Arguments (logical)   | On success  | Short description
-------|----------------|-----------------------|-------------|-------------------
0x00  x| exit           | status_code           | !           | Terminate the calling process
0x01  x| get_pid        |                       | pid         | Get pid of the calling process
0x02  x| debug_print    | str_len, *str_ptr*    | -           | Print a UTF-8 string to the kernel terminal
0x03  x| mem_set_size   | total_bytes           | total_bytes | Set dynamic memory size, rounds up to page size
0x30  x| fs_open        | **path**              | fd          | Open a file from vfs
0x31  x| fs_exec        | **path**              | fd          | Execute a file from vfs
0x32  x| fs_attach      | **path**, is_leaf?    | fd          | Create dynamic fs node (empty path for unnamed)
0x33  x| fs_fileinfo    | **path**, *dst*       | *FileInfo*  | Get metadata about a file
0x40   | fd_close       | fd                    | -           | Close fd, closing a mount unmounts
0x41  x| fd_read        | fd, *buf*, count      | ReadResult  | Reads `count` bytes from `fd` to `buf_ptr`
0x42  x| fd_write       | fd, *buf*, count      | byte_count  | Writes `count` bytes from `buf_ptr` to `fd`
0x43   | fd_synchronize | fd                    | -           | Ensures all written data has been delivered
0x44   | fd_control     | fd, function          | -           | Send control function for a file
0x45  x| fd_select      | **fds**, blocking?    | fd          | Wait until first fd is available
0x46  x| fd_get_pid     | fd                    | pid         | Maps file descriptor to a process id
0x50  x| sched_yield    | -                     | -           | Yield control to schedule next process
0x51  x| sched_sleep_ns | ns                    | -           | Sleep specified number of nanoseconds

*Cursived* text implies that something is a pointer.
**Bold** text implies that something is a read-only slice, i.e. `len, ptr` pair.
Values like `ok?` ending with `?` represent booleans.

# Call structure

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
