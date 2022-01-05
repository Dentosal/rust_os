use libd7::{ipc, net::d7net::*};

use crate::NET_STATE;

pub fn handle_arp_packet(frame: &ethernet::Frame, arp_packet: &arp::Packet) {
    // Update arp table
    if arp_packet.src_ip != Ipv4Addr::ZERO {
        println!(
            "ARP: Mark owner {:?} {:?}",
            arp_packet.src_ip, arp_packet.src_hw
        );
        {
            let mut net_state = NET_STATE.write();
            net_state
                .arp_table
                .insert(arp_packet.src_ip, arp_packet.src_hw);
        }

        if arp_packet.is_request() {
            // Reply to mac-targeted ARP packets if the corresponding interface has an ip
            if arp_packet.dst_hw != MacAddr::ZERO {
                let net_state = NET_STATE.read();

                if let Some(intf) = net_state.interface(arp_packet.dst_hw) {
                    if !intf.arp_probe_ok {
                        return;
                    }
                    if let Some(ip) = intf.settings.ipv4 {
                        if arp_packet.dst_ip == ip {
                            println!("ARP: Replying");

                            let reply = (ethernet::Frame {
                                header: ethernet::FrameHeader {
                                    dst_mac: frame.header.src_mac,
                                    src_mac: intf.mac_addr,
                                    ethertype: EtherType::ARP,
                                },
                                payload: arp_packet.to_reply(intf.mac_addr, ip).to_bytes(),
                            })
                            .to_bytes();

                            ipc::publish("nic/send", &reply).unwrap();
                        }
                    }
                }
            } else if arp_packet.dst_ip != Ipv4Addr::ZERO {
                // Reply to ip-targeted ARP packets if the corresponding interface exists
                let net_state = NET_STATE.read();
                for intf in &net_state.interfaces {
                    if !intf.arp_probe_ok {
                        continue;
                    }
                    if let Some(ip) = intf.settings.ipv4 {
                        if arp_packet.dst_ip == ip {
                            println!("ARP: Replying");

                            let reply = (ethernet::Frame {
                                header: ethernet::FrameHeader {
                                    dst_mac: frame.header.src_mac,
                                    src_mac: intf.mac_addr,
                                    ethertype: EtherType::ARP,
                                },
                                payload: arp_packet.to_reply(intf.mac_addr, ip).to_bytes(),
                            })
                            .to_bytes();

                            ipc::publish("nic/send", &reply).unwrap();
                            break;
                        }
                    }
                }
            }
        }
    }
}
