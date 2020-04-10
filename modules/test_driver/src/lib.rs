#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![deny(unused_must_use)]

use libd7::{
    attachment::*,
    console::Console,
    fs::{list_dir, File},
    process::Process,
    syscall,
};

#[macro_use]
extern crate alloc;

use alloc::prelude::v1::*;

#[no_mangle]
fn main() -> u64 {
    let pid = syscall::get_pid();

    // Start ATA PIO driver
    let p = Process::spawn("/mnt/staticfs/mod_ata_pio").unwrap();

    // Wait until the device endpoint is available
    loop {
        let dirlist = list_dir("/dev").unwrap();
        if dirlist.contains(&"ata_pio_0".to_owned()) {
            break;
        }

        // syscall::debug_print("Waiting...");
        syscall::sched_sleep_ns(1_000_000).unwrap();
    }

    let drive = File::open("/dev/ata_pio_0").unwrap();

    // Example: Read boot sector and verify signature
    let mut buffer = [0; 512];
    let count = drive.read(&mut buffer).unwrap();
    assert_eq!(count, buffer.len());

    if buffer[510..512] == [0x55, 0xaa] {
        syscall::debug_print("Correct boot signature");
    } else {
        syscall::debug_print("Incorrect boot signature");
    }

    // The spawned process will be killed as this one terminates


    // Start networking daemon driver
    let p_netd = Process::spawn("/mnt/staticfs/netd").unwrap();


    // Console
    let mut console = Console::open(
        "/dev/console",
        "/mnt/staticfs/keycode.json",
        "/mnt/staticfs/keymap.json",
    )
    .unwrap();
    loop {
        syscall::debug_print(&format!("Input test:"));
        let line = console.read_line().unwrap();
        syscall::debug_print(&format!("Line {:?}", line));
        if line == "exit" {
            break;
        }
    }

    0
}
