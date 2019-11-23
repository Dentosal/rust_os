#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![deny(unused_must_use)]

use libd7::syscall;

#[macro_use]
extern crate alloc;

#[no_mangle]
fn main() -> u64 {
    // Test: get pid and use it as exit code
    let pid = syscall::get_pid();

    let fileinfo = syscall::fs_fileinfo("/").unwrap();
    syscall::debug_print(&format!("Fileinfo / : {:?}", fileinfo));

    let mut buffer = [0; 64];

    let root_fd = syscall::fs_open("/").unwrap();
    let count = syscall::fd_read(root_fd, &mut buffer).unwrap();
    syscall::debug_print(&format!("/ : {:?}", &buffer[..count]));

    let fd = syscall::fs_open("/dev/zero").unwrap();
    let count = syscall::fd_read(fd, &mut buffer).unwrap();
    syscall::debug_print(&format!("/dev/zero : {:?}", &buffer[..count]));

    let fd = syscall::fs_open("/dev/null").unwrap();
    let count = syscall::fd_read(fd, &mut buffer).unwrap();
    syscall::debug_print(&format!("/dev/null : {:?}", &buffer[..count]));

    if pid == 0 {
        let fd = syscall::fs_open("/dev/test").unwrap();
        let count = syscall::fd_read(fd, &mut buffer).unwrap();
        syscall::debug_print(&format!("/dev/test : {:?}", &buffer[..count]));
    } else {
        for i in 0..10 {
            syscall::debug_print(&format!("iter = {}", i));
            syscall::sched_sleep_ns(500_000_000).unwrap();
        }
    }

    pid
}
