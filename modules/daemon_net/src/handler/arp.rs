use libd7::net::d7net::{arp, ethernet, EtherType, Ipv4Addr};

use super::Handler;
use crate::NetState;

// Sends replies to ARP packets
pub struct ArpHandler;

impl ArpHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Handler for ArpHandler {
    fn on_receive(&mut self, net: &mut NetState, frame: &ethernet::Frame) {
        let my_ip = net.my_info.ipv4.unwrap();

        let arp_packet = arp::Packet::from_bytes(&frame.payload);
        println!("RECV ARP {:?}", arp_packet);

        // Update ARP cache, ignore only ARP probes
        if arp_packet.sender_ip != Ipv4Addr::ZERO {
            let mut cache = crate::arp_cache::ARP_CACHE.try_lock().unwrap();
            cache.insert(arp_packet.sender_ip.to_ipv6(), arp_packet.sender_hw);
        }

        // Reply to packets directed to this host
        if arp_packet.is_request() && arp_packet.target_ip == my_ip {
            println!("ARP: Replying");

            crate::send_frame(ethernet::Frame {
                header: ethernet::FrameHeader {
                    dst_mac: frame.header.src_mac,
                    src_mac: net.my_info.mac,
                    ethertype: EtherType::Arp,
                },
                payload: arp_packet.to_reply(net.my_info.mac, my_ip).to_bytes(),
            })
            .unwrap();
        }
    }
}
