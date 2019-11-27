#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![deny(unused_must_use)]

use libd7::{fs, syscall, process::Process};

#[macro_use]
extern crate alloc;

#[no_mangle]
fn main() -> u64 {
    let pid = syscall::get_pid();

    // List processes
    syscall::debug_print(&format!("{:?}", fs::list_dir("/prc").unwrap()));

    if pid < 5 {
        let p = Process::spawn("/mnt/staticfs/mod_test").unwrap();
        let retcode = p.wait();
        syscall::debug_print(&format!("RETC {:?}", retcode));
        syscall::sched_sleep_ns(500_000_000).unwrap();
    }

    pid
}
