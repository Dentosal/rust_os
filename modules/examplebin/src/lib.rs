#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use libd7::{
    net::{d7net::*, tcp::TcpListener},
    syscall,
};

use alloc::prelude::v1::*;

#[no_mangle]
fn main() {
    let pid = syscall::get_pid();

    let tcp_server =
        TcpListener::listen(IpAddr::V4(Ipv4Addr::ZERO), 80).expect("Could not open socket");

    loop {
        let conn = tcp_server.accept().unwrap();
        println!("New connection from {:?}", conn.remote());

        let bytes = conn.receive().unwrap();
        let msg = String::from_utf8(bytes).unwrap();
        println!("Message {:?}", msg);

        conn.send(&"Hello, client!\n".as_bytes()).unwrap();
    }
}
