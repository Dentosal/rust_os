use spin::Mutex;

use libd7::{
    ipc,
    net::{
        d7net::{
            ethernet, ipv4, ipv_either, EtherType, IpAddr, IpProtocol, Ipv4Addr, Ipv6Addr, MacAddr,
        },
        socket::{SocketDescriptor, SocketOptions},
    },
    prelude::*,
    syscall::SyscallResult,
};

use crate::my_info::MyInfo;

pub fn send_frame(frame: ethernet::Frame) -> Result<(), ()> {
    println!("SEND {:?}", frame);
    let data = frame.to_bytes();
    ipc::deliver("nic/send", &data).map_err(|_| ())
}

pub fn send_ip_packet<F>(
    my: &MyInfo,
    target_ip: Ipv6Addr,
    protocol: IpProtocol,
    f: F,
) -> Result<(), ()>
where
    F: FnOnce(&ipv_either::Header) -> Vec<u8>,
{
    let dst_mac = {
        let cache = crate::arp_cache::ARP_CACHE.try_lock().unwrap();
        cache.get(target_ip).expect("Missing target mac")
    };

    match target_ip.to_generic() {
        IpAddr::V4(ip) => {
            let header = ipv4::Header::new(protocol, my.ipv4.ok_or(())?, ip);
            let payload = ipv4::Packet {
                header,
                payload: f(&ipv_either::Header::V4(header)),
            }
            .to_bytes();
            send_frame(ethernet::Frame {
                header: ethernet::FrameHeader {
                    dst_mac,
                    src_mac: my.mac,
                    ethertype: EtherType::Ipv4,
                },
                payload,
            })
        }
        IpAddr::V6(ip) => todo!("ipv6 support"),
    }
}
