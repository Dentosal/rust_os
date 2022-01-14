#![no_std]
#![feature(allocator_api)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::convert::TryInto;

#[macro_use]
extern crate libd7;

use libd7::{
    // console::Console,
    net::tcp,
    service,
    syscall,
};

#[no_mangle]
fn main() -> u64 {
    let pid = syscall::get_pid();

    // Wait until netd is available
    println!("Wait for netd >");
    service::wait_for_one("netd");
    println!("Wait for netd <");

    syscall::sched_sleep_ns(2_000_000_000).unwrap();

    if let Err(err) = main_inner() {
        println!("Error: {:?}", err);
        return 1;
    }

    return 0;
}

fn main_inner() -> Result<(), tcp::Error> {
    println!("Connect");
    let socket = tcp::Stream::connect("example.org:80")?;
    println!("Send request");
    socket.send(b"GET / HTTP/1.1\r\nHost: example.org\r\nConnection: close\r\n\r\n")?;
    println!("Shutdown");
    socket.shutdown()?;
    println!("Read response");
    let mut fetched = String::new();
    let mut buffer = [0; 1024];
    loop {
        let n = socket.recv(&mut buffer)?;
        fetched.push_str(core::str::from_utf8(&buffer[..n]).expect("Invalid utf-8"));
        if n < buffer.len() {
            break;
        }
    }

    println!("reply {}", fetched);
    socket.close()?;

    Ok(())

    // // Console
    // let mut console = Console::open(
    //     "/dev/console",
    //     "/mnt/staticfs/keycode.json",
    //     "/mnt/staticfs/keymap.json",
    // )
    // .unwrap();
    // loop {
    //     syscall::debug_print(&format!("Input test:"));
    //     let line = console.read_line().unwrap();
    //     syscall::debug_print(&format!("Line {:?}", line));
    //     if line == "exit" {
    //         break;
    //     }
    // }

    // 0
}
