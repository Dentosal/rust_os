use core::convert::TryFrom;
use core::hint::unreachable_unchecked;
use core::mem::MaybeUninit;
use core::time::Duration;

use d7abi::{fs::{FileDescriptor, FileInfo}, SyscallErrorCode, SyscallNumber};

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

    asm!("int 0xd7"
        : "={rax}"(success), "={rdi}"(result)
        :
            "{rax}"(number),
            "{rdi}"(args.0),
            "{rsi}"(args.1),
            "{rdx}"(args.2),
            "{rcx}"(args.3)
        : "memory"
        : "volatile", "intel"
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
        asm!("int 0xd7" :: "{rax}"(SyscallNumber::exit), "{rdi}"(return_code) :: "intel");
        unreachable_unchecked();
    }
}

/// This system call never fails
pub fn get_pid() -> u64 {
    unsafe { syscall!(SyscallNumber::get_pid).unwrap() }
}

/// This system call never fails
pub fn debug_print(s: &str) {
    let len = s.len() as u64;
    let slice = s.as_ptr() as u64;
    unsafe {
        syscall!(SyscallNumber::debug_print; len, slice).unwrap();
    }
}

/// # Safety
/// Can be used to confuse thé memory manager, and generally
/// should not be used outside of this library.
pub unsafe fn mem_set_size(new_size_bytes: u64) -> SyscallResult<u64> {
    syscall!(SyscallNumber::mem_set_size; new_size_bytes)
}


pub fn fs_open(path: &str) -> SyscallResult<FileDescriptor> {
    let len = path.len() as u64;
    let slice = path.as_ptr() as u64;

    unsafe {
        Ok(FileDescriptor::from_u64(
            syscall!(SyscallNumber::fs_open; len, slice)?,
        ))
    }
}

/// Like fs_open, but executes the file instead
pub fn fs_exec(path: &str) -> SyscallResult<FileDescriptor> {
    let len = path.len() as u64;
    let slice = path.as_ptr() as u64;

    unsafe {
        Ok(FileDescriptor::from_u64(
            syscall!(SyscallNumber::fs_exec; len, slice)?,
        ))
    }
}

/// Like fs_open, but attaches to the file instead
pub fn fs_attach(path: &str, is_leaf: bool) -> SyscallResult<FileDescriptor> {
    let len = path.len() as u64;
    let slice = path.as_ptr() as u64;

    unsafe {
        Ok(FileDescriptor::from_u64(
            syscall!(SyscallNumber::fs_attach; len, slice, is_leaf as u64)?,
        ))
    }
}

pub fn fs_fileinfo(path: &str) -> SyscallResult<FileInfo> {
    let len = path.len() as u64;
    let slice = path.as_ptr() as u64;

    let mut info: MaybeUninit<FileInfo> = MaybeUninit::uninit();

    unsafe {
        let _ = syscall!(SyscallNumber::fs_fileinfo; len, slice, info.as_mut_ptr() as u64)?;
        Ok(info.assume_init())
    }
}


pub fn fd_read(fd: FileDescriptor, buf: &mut [u8]) -> SyscallResult<usize> {
    unsafe {
        Ok(syscall!(
            SyscallNumber::fd_read;
            fd.as_u64(), buf.as_mut_ptr() as u64, buf.len() as u64
        )? as usize)
    }
}

pub fn fd_write(fd: FileDescriptor, buf: &[u8]) -> SyscallResult<usize> {
    unsafe {
        Ok(syscall!(
            SyscallNumber::fd_write;
            fd.as_u64(), buf.as_ptr() as u64, buf.len() as u64
        )? as usize)
    }
}

pub fn fd_select(fds: &[FileDescriptor], timeout: Option<Duration>) -> SyscallResult<FileDescriptor> {
    unsafe {
        Ok(FileDescriptor::from_u64(syscall!(
            SyscallNumber::fd_select;
            fds.len() as u64,
            fds.as_ptr() as u64,
            timeout.map(|d| d.as_nanos() as u64).unwrap_or(0)
        )?))
    }
}

/// This system call never fails, and does not return anything
pub fn sched_yield() {
    let _ = unsafe { syscall!(SyscallNumber::sched_yield) };
}

/// Max sleep time is 2**64 ns, about 584 years.
pub fn sched_sleep_ns(ns: u64) -> SyscallResult<()> {
    unsafe { syscall!(SyscallNumber::sched_sleep_ns; ns).map(|_| ()) }
}
