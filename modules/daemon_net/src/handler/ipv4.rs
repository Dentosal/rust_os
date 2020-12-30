//! TODO: packet deframentation

use libd7::prelude::*;

use libd7::net::d7net::{ethernet, ipv4, ipv_either, tcp, IpProtocol, Ipv4Addr, Ipv6Addr};

use super::Handler;
use crate::NetState;

pub struct Ipv4Handler;
impl Ipv4Handler {
    pub fn new() -> Self {
        Self
    }
}

impl Handler for Ipv4Handler {
    fn on_receive(&mut self, net: &mut NetState, frame: &ethernet::Frame) {
        let ip_packet = ipv4::Packet::from_bytes(&frame.payload);
        let src_ip = ip_packet.header.src_ip.to_ipv6();
        let dst_ip = ip_packet.header.dst_ip.to_ipv6();
        println!("RECV v4 {:?}", ip_packet);

        let reply = match ip_packet.header.protocol {
            IpProtocol::TCP => {
                let tcp_packet = tcp::Segment::from_bytes(&ip_packet.payload);
                println!("RECV tcp {:?}", tcp_packet);
                let src_port = tcp_packet.header.src_port;
                let dst_port = tcp_packet.header.dst_port;
                let reply = net.modify_sockets(|net, sockets| {
                    if let Some(socket) = sockets.tcp.get_mut(&(dst_ip, dst_port)) {
                        println!("Socket {:?} receive", (dst_ip, dst_port));
                        socket.on_receive(
                            (dst_ip, dst_port),
                            (dst_ip, dst_port),
                            (src_ip, src_port),
                            tcp_packet,
                        )
                    } else if let Some(socket) = sockets.tcp.get_mut(&(Ipv6Addr::ZERO, dst_port)) {
                        println!("Socket {:?} receive", (Ipv6Addr::ZERO, dst_port));
                        socket.on_receive(
                            (Ipv6Addr::ZERO, dst_port),
                            (dst_ip, dst_port),
                            (src_ip, src_port),
                            tcp_packet,
                        )
                    } else {
                        println!("Unknown socket {:?}, ignoring", (dst_ip, dst_port));
                        None
                    }
                });

                if let Some(reply_segment) = reply {
                    let header = ipv4::Header::new(
                        IpProtocol::TCP,
                        net.my_info.ipv4.unwrap(),
                        ip_packet.header.src_ip,
                    );
                    let packet = ipv4::Packet {
                        header,
                        payload: reply_segment.to_bytes(&ipv_either::Header::V4(header)),
                    };
                    Some(packet)
                } else {
                    None
                }
            }
            _ => {
                println!("Unknown protocol, ignoring");
                None
            }
        };

        if let Some(reply) = reply {
            let reply_frame = ethernet::Frame {
                header: ethernet::FrameHeader {
                    src_mac: net.my_info.mac,
                    dst_mac: frame.header.src_mac,
                    ethertype: frame.header.ethertype,
                },
                payload: reply.to_bytes(),
            };
            crate::send_frame(reply_frame).unwrap();
        }
    }
}
