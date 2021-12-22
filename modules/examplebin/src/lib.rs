#![no_std]
#![feature(allocator_api)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

use alloc::vec::Vec;

#[macro_use]
extern crate libd7;

use libd7::{
    // attachment::*,
    // console::Console,
    // fs::{list_dir, File},
    // process::Process,
    ipc,
    net::d7net::*,
    // net::tcp,
    service,
    syscall,
};

#[no_mangle]
fn main() -> u64 {
    let pid = syscall::get_pid();

    // let tcp_server = tcp::Socket::bind(SocketAddr {
    //     host: IpAddr::V4(Ipv4Addr([0,0,0,0])),
    //     port: 22,
    // }).expect("Could not open socket");

    // Wait until netd is available
    println!("Wait for netd >");
    service::wait_for_one("netd");
    println!("Wait for netd <");

    0

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
    //     } else {
    //         let dirlist = list_dir("/net").unwrap();
    //         syscall::debug_print(&format!("/net: {:?}", dirlist));
    //     }
    // }

    // 0
}
