use core::hint::unreachable_unchecked;

#[allow(non_camel_case_types)]
#[repr(u64)]
pub enum SyscallNumber {
    exit = 0x00,
    get_pid = 0x01,
    debug_print = 0x02,
    mem_set_size = 0x03,
}

macro_rules! syscall {
    ($n:expr; $a0:expr, $a1:expr, $a2:expr, $a3:expr) => {
        syscall($n as u64, ($a0, $a1, $a2, $a3))
    };
    ($n:expr; $a0:expr, $a1:expr, $a2:expr) => {syscall!($n; $a0, $a1, $a2, 0)};
    ($n:expr; $a0:expr, $a1:expr) => {syscall!($n; $a0, $a1, 0, 0)};
    ($n:expr; $a0:expr) => {syscall!($n; $a0, 0, 0, 0)};
    ($n:expr) => {syscall!($n; 0, 0, 0, 0)};
}

/// # Safety
/// Allows any unsafe system call to be called, and doesn't protect from invalid arguments.
pub unsafe fn syscall(number: u64, args: (u64, u64, u64, u64)) -> Result<u64, u64> {
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
        :
        : "intel"
    );

    if success == 1 {
        Ok(result)
    } else if success == 0 {
        Err(result)
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

pub fn get_pid() -> u64 {
    unsafe { syscall!(SyscallNumber::get_pid).unwrap() }
}

pub fn debug_print(s: &str) -> Result<u64, u64> {
    let len = s.len() as u64;
    let slice = s.as_ptr() as u64;
    unsafe { syscall!(SyscallNumber::debug_print; len, slice) }
}


/// # Safety
/// Can be used to confuse memory manager, and generally shouldn't
/// be used outside this library.
pub unsafe fn mem_set_size(new_size_bytes: u64) -> Result<u64, u64> {
    syscall!(SyscallNumber::mem_set_size; new_size_bytes)
}
