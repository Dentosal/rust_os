#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![deny(unused_must_use)]

use d7abi::syscall;

#[macro_use]
extern crate alloc;

#[no_mangle]
fn main() -> u64 {
    // Test: get pid and use it as exit code
    let pid = syscall::get_pid();

    // syscall::debug_print(&format!("My pid is {}", pid)).unwrap();

    for i in 0..100 {
        syscall::debug_print(&format!("iter = {}", i)).unwrap();
        syscall::clock_sleep_ns(100_000_000 * (1 + pid)).unwrap();
    }

    pid
}
