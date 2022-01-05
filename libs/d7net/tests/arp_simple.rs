use d7net::*;

#[test]
fn arp_simple() {
    let mac_addr: MacAddr = MacAddr::from_bytes(&[1, 2, 3, 4, 5, 6]);

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

    let arpp = arp::Packet {
        ptype: EtherType::Ipv4,
        operation: arp::Operation::Request,
        src_hw: mac_addr,
        src_ip: Ipv4Addr::from_bytes(&[192, 168, 10, 15]),
        dst_hw: MacAddr::ZERO,
        dst_ip: Ipv4Addr::from_bytes(&[192, 168, 10, 1]),
    };

    let ef = ethernet::Frame {
        header: ethernet::FrameHeader {
            dst_mac: MacAddr::BROADCAST,
            src_mac: mac_addr,
            ethertype: EtherType::ARP,
        },
        payload: arpp.to_bytes(),
    };

    let mut ef_packet = ef.to_bytes();
    while ef_packet.len() < 64 {
        ef_packet.push(0);
    }

    assert_eq!(packet, ef_packet);
}
