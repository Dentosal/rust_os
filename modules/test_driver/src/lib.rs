#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![deny(unused_must_use)]

use libd7::{syscall, process::Process};

#[macro_use]
extern crate alloc;

#[no_mangle]
fn main() -> u64 {
    // Test: get pid and use it as exit code
    let pid = syscall::get_pid();

    // let fileinfo = syscall::fs_fileinfo("/").unwrap();
    // syscall::debug_print(&format!("Fileinfo / : {:?}", fileinfo));

    // let fd = syscall::fs_open("/dev/zero").unwrap();
    // let count = syscall::fd_read(fd, &mut buffer).unwrap();
    // syscall::debug_print(&format!("/dev/zero : {:?}", &buffer[..count]));

    if pid < 5 {
        let p = Process::spawn("/mnt/staticfs/mod_test").unwrap();
        let retcode = p.wait();
        syscall::debug_print(&format!("RETC {:?}", retcode));
        syscall::sched_sleep_ns(500_000_000).unwrap();
    }

    pid * 2
}
