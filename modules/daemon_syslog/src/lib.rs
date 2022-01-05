//! Syslog daemon.
//! Combines kernel and service logs, writes to disk and console.
//!
//! TODO: more find-grained system calls, to only remove the data when it
//! has been written on the disk.

#![no_std]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::borrow::ToOwned;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use libd7::{ipc, process::ProcessId, select, syscall};

#[no_mangle]
fn main() -> ! {
    println!("Syslog daemon starting");

    let mut read_buffer = [0u8; 0x1_0000];
    let mut send_buffer: String = String::new();
    let mut line_buffer: Vec<u8> = Vec::new();

    // Inform the serviced that we are up
    libd7::service::register("syslogd", false);

    loop {
        let count = syscall::kernel_log_read(&mut read_buffer).unwrap();
        if count != 0 {
            for &byte in &read_buffer[..count] {
                if byte == b'\n' {
                    send_buffer.push_str(core::str::from_utf8(&line_buffer).unwrap());
                    send_buffer.push('\n');
                    line_buffer.clear();
                } else {
                    line_buffer.push(byte);
                }
            }

            if send_buffer.len() > 0 {
                ipc::deliver("console/kernel_log", &send_buffer).unwrap();
                send_buffer.clear();
            }

            assert!(line_buffer.len() < 1000, "Line buffer overflow");
        }

        // Sleep if there is no buffer left
        if count < read_buffer.len() {
            // TODO: increase poll frequency
            syscall::sched_sleep_ns(1_000_000_000).unwrap();
        }
    }
}
