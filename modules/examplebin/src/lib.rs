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

    for i in 0..100 {
        syscall::debug_print(&format!("TICK {} - {}", pid, i));
        break;
        syscall::sched_sleep_ns(1_000_000_000).unwrap();
    }

    // Wait until netd is available
    println!("Wait for netd >");
    service::wait_for_one("netd");
    println!("Wait for netd <");

    let mac_addr: MacAddr = match ipc::request("netd/mac", &()) {
        Ok(mac) => mac,
        Err(err) => panic!("NIC ping failed {:?}", err),
    };

    let arpp = arp::Packet {
        ptype: EtherType::Ipv4,
        operation: arp::Operation::Request,
        sender_hw: mac_addr,
        sender_ip: Ipv4Addr::from_bytes(&[192, 178, 10, 16]),
        target_hw: MacAddr::ZERO,
        target_ip: Ipv4Addr::from_bytes(&[192, 168, 10, 1]),
    };

    let ef = ethernet::Frame {
        header: ethernet::FrameHeader {
            dst_mac: MacAddr::BROADCAST,
            src_mac: mac_addr,
            ethertype: EtherType::ARP,
        },
        payload: arpp.to_bytes(),
    };

    let mut packet = ef.to_bytes();
    while packet.len() < 64 {
        packet.push(0);
    }

    ipc::deliver("nic/send", &packet).expect("Delivery failed");

    // let ef = ethernet::Frame {
    //     header: ethernet::FrameHeader {
    //         dst_mac: MacAddr::BROADCAST,
    //         src_mac: mac_addr,
    //         ethertype: EtherType::Ipv4,
    //     },
    //     payload: builder::ipv4_udp::Builder::new(
    //         Ipv4Addr::ZERO,
    //         Ipv4Addr::BROADCAST,
    //         68,
    //         67,
    //         dhcp::Payload::discover(xid, mac_addr).to_bytes(),
    //     )
    //     .build(),
    // };

    // let mut packet = ef.to_bytes();
    // while packet.len() < 64 {
    //     packet.push(0);
    // }

    // ipc::deliver("nic/send", &packet).expect("Delivery failed");

    println!("Delivered");

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
