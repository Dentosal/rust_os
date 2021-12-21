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
    service,
    syscall,
    net::{d7net::*},
    // net::tcp,
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
        // break;
        syscall::sched_sleep_ns(1_000_000_000).unwrap();
    }


    // Wait until netd is available
    println!("Wait for netd >");
    service::wait_for_one("netd");
    println!("Wait for netd <");

    let mac_addr: MacAddr = match ipc::request("nic/rtl8139/mac", &()) {
        Ok(mac) => mac,
        Err(err) => panic!("NIC ping failed {:?}", err),
    };

    // ARP

    let mut packet = Vec::new();

    // dst mac: broadcast
    packet.extend(&MacAddr::BROADCAST.0);

    // src mac: this computer
    packet.extend(&mac_addr.0);

    // ethertype: arp
    packet.extend(&EtherType::ARP.to_bytes());

    // arp: HTYPE: ethernet
    packet.extend(&1u16.to_be_bytes());

    // arp: PTYPE: ipv4
    packet.extend(&0x0800u16.to_be_bytes());

    // arp: HLEN: 6 for mac addr
    packet.push(6);

    // arp: PLEN: 4 for ipv4
    packet.push(4);

    // arp: Opeeration: request
    packet.extend(&1u16.to_be_bytes());

    // arp: SHA: our mac
    packet.extend(&mac_addr.0);

    // arp: SPA: our ip (hardcoded for now)
    packet.extend(&[192, 168, 10, 15]);

    // arp: THA: target mac, ignored
    packet.extend(&[0, 0, 0, 0, 0, 0]);

    // arp: TPA: target ip (bochs vnet router)
    packet.extend(&[192, 168, 10, 1]);

    // padding
    while packet.len() < 64 {
        packet.push(0);
    }

    ipc::deliver("nic/send", &packet).expect("Delivery failed");

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
