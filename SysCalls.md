System call reference
=====================

# List

Number | Name           | Arguments (logical) | On success  |Can fail| Short description
-------|----------------|---------------------|-------------|--------|-------------------
0x00   | exit           | error_code          | !           | No     | Terminate the calling process
0x01   | get_pid        |                     | pid         | No     | Get pid of the calling process
0x10   | kill           | pid, method         | Status code | Yes    | Terminate a process
0x11   | proc_exec      | path, args          | pid         | Yes    | Execute a program
0x11   | proc_wait      | pid, state, first?  | state       | Yes    | Wait for specific process state
0x30   | fs_fileinfo    | path                | FileInfo    | Yes    | Get metadata about a file
0x31   | fs_open        | path, flags         | fd          | Yes    | Open (or create and open) a file from vfs
0x32   | fs_delete      | path                | -           | Yes    | Delete a file
0x33   | fs_rename      | old_path, new_path  | -           | Yes    | Rename or move a file
0x33   | fs_copy        | path, new_path      | -           | Yes    | Copy a file
0x1a   | fs_lock        | exclusive?, block?  | lock_fd     | Yes    | Lock a file
0x1b   | fs_unlock      | lock_fd             | -           | Yes    | Unlock a file
0x40   | fd_close       | fd                  | fd          | Yes    | Close fd
0x41   | fd_read        | fd, buf_ptr, count  | byte_count  | Yes    | Reads `count` bytes from `fd` to `buf_ptr`
0x42   | fd_write       | fd, buf_ptr, count  | byte_count  | Yes    | Writes `count` bytes from `buf_ptr` to `fd`
0x43   | fd_seek_abs    | fd, position: u64   | -           | Yes    | Absolute seek into `fb`
0x44   | fd_seek_rel    | fd, offset: i64     | -           | Yes    | Relative seek into `fb`
0x45   | fd_info        | fd                  | FdInfo      | Yes    | Info about fd, like lock status, file type, etc.
0x46   | fd_get_flags   | fd                  | FdFlags     | Yes    | Get fd flags, like blocking or not
0x47   | fd_set_flags   | fd, FdFlags         | FdFlags     | Yes    | Set fd flags, like blocking or not
0x48   | fd_poll        | fd, TBD             | TODO        | Yes    | Polls list of fds
0x49   | fd_select      | fd, TBD             | TODO        | Yes    | Polls list of fds
0x50   | sched_yield    | -                   | -           | No     | Yield control to schedule next process
0x60   | clock_sleep_ns | ns                  | -           | No     | Sleep specified number of nanoseconds
0x61   | clock_get_ts   | -                   | timestamp   | No     | Get timestamp
0x62   | clock_get_rt   | -                   | real_time   | No     | Get current real-world time
0x63   | clock_set_rt   | real_time           | -           | Yes    | Set current real-world time


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
