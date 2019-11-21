System call reference
=====================

# List

Number | Name           | Arguments (logical) | On success  | Short description
-------|----------------|---------------------|-------------|-------------------
0x00   | exit           | status_code         | !           | Terminate the calling process
0x01   | get_pid        |                     | pid         | Get pid of the calling process
0x02   | debug_print    | str_len, str_ptr    | -           | Print a UTF-8 string to the kernel terminal
0x03   | mem_set_size   | total_bytes         | total_bytes | Set dynamic memory size, rounds up to page size
0x10   | proc_kill      | pid, method         | StatusCode  | Terminate a process
0x11   | proc_exec      | path, args          | pid         | Execute a program
0x11   | proc_wait      | pid, state, first?  | state       | Wait for specific process state
0x30   | fs_fileinfo    | path                | FileInfo    | Get metadata about a file
0x31   | fs_open        | path, flags         | fd          | Open (or create and open) a file from vfs
0x32   | fs_delete      | path                | -           | Delete a file
0x33   | fs_rename      | old_path, new_path  | -           | Rename or move a file
0x33   | fs_copy        | path, new_path      | -           | Copy a file
0x1a   | fs_lock        | exclusive?, block?  | lock_fd     | Lock a file
0x1b   | fs_unlock      | lock_fd             | -           | Unlock a file
0x40   | fd_close       | fd                  | fd          | Close fd
0x41   | fd_read        | fd, buf_ptr, count  | byte_count  | Reads `count` bytes from `fd` to `buf_ptr`
0x42   | fd_write       | fd, buf_ptr, count  | byte_count  | Writes `count` bytes from `buf_ptr` to `fd`
0x43   | fd_seek_abs    | fd, position: u64   | -           | Absolute seek into `fb`
0x44   | fd_seek_rel    | fd, offset: i64     | -           | Relative seek into `fb`
0x45   | fd_info        | fd                  | FdInfo      | Info about fd, like lock status, file type, etc.
0x46   | fd_get_flags   | fd                  | FdFlags     | Get fd flags, like blocking or not
0x47   | fd_set_flags   | fd, FdFlags         | FdFlags     | Set fd flags, like blocking or not
0x48   | fd_poll        | fd, TBD             | TODO        | Polls list of fds
0x49   | fd_select      | fd, TBD             | TODO        | Polls list of fds
0x50   | sched_yield    | -                   | -           | Yield control to schedule next process
0x51   | sched_wait     | TBD, (timeout)      | -           | Wait for some event to occur
0x60   | clock_sleep_ns | ns                  | -           | Sleep specified number of nanoseconds
0x6a   | clock_get_ts   | -                   | timestamp   | Get timestamp
0x6b   | clock_get_rt   | -                   | real_time   | Get current real-world time
0x6c   | clock_set_rt   | real_time           | -           | Set current real-world time
0x70   | mount          | path                | mount_id    | Create a fs node and manage fs under it
0x71   | unmount        | mount_id            | -           | Remove a mount


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
