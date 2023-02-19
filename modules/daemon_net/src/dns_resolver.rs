use alloc::string::String;
use alloc::vec::Vec;

use libd7::net::d7net::*;
use libd7::{ipc, random};

use crate::{ports, NET_STATE};

/// TODO: make configurable
const DEFAULT_NAMESERVERS: &[IpAddr] = &[
    IpAddr::V4(Ipv4Addr([1, 1, 1, 1])),
    IpAddr::V4(Ipv4Addr([1, 0, 0, 1])),
];

pub type Query = (String, dns::QueryType);
pub type Answer = Result<Vec<dns::QueryResult>, dns::NxDomain>;

pub struct DnsResolver {
    servers: Vec<IpAddr>,
    /// TODO: cache?
    /// TODO: timeout and retrying
    pending_requests: Vec<(u16, Query, ipc::ReplyCtx<Answer>)>,
}

impl DnsResolver {
    pub fn new() -> Self {
        Self {
            servers: DEFAULT_NAMESERVERS.to_vec(),
            pending_requests: Vec::new(),
        }
    }

    pub fn on_packet(&mut self, p: udp::Packet) {
        match dns::parse_reply(&p.payload) {
            Ok(reply) => {
                // Resolve user requests
                self.pending_requests
                    .drain_filter(|(req_id, q, _)| (*req_id, &*q) == (reply.req_id, &reply.query))
                    .for_each(|(_, _, rctx)| {
                        let _ = rctx.reply(
                            reply
                                .records
                                .clone()
                                .map(|v| v.into_iter().map(|(_, _, c)| c).collect()),
                        ); // Ignore caller errors
                    });
            },
            Err(err) => log::warn!("DNS server replied with an error {:?}", err),
        }
    }

    pub fn user_resolve(&mut self, rctx: ipc::ReplyCtx<Answer>, query: Query) {
        log::debug!("Resolve {:?}", query);

        let req_id = u16::from_le_bytes(random::fast_arr());
        let r = try_send(
            self.servers[0],
            dns::make_question(req_id, &query.0, query.1),
        );

        match r {
            Ok(()) => {
                self.pending_requests.push((req_id, query, rctx));
            },
            Err(SendError) => {
                log::warn!("Send failed");
                let _ = rctx.nack(); // Ignore caller errors
            },
        }
    }
}

fn try_send(dst_ip: IpAddr, payload: Vec<u8>) -> Result<(), SendError> {
    let (dst_mac, src_mac, src_ip) = {
        let net_state = NET_STATE.try_read().expect("NET_STATE locked");
        let intf = net_state.default_send_interface().ok_or(SendError)?;
        let router_ip = intf.settings.routers.first().ok_or(SendError)?;
        let router_mac = net_state.arp_table.get(router_ip).ok_or(SendError)?;
        let ip_addr = intf.settings.ipv4.ok_or(SendError)?;
        (*router_mac, intf.mac_addr, ip_addr)
    };

    let udp_payload = builder::ipv4_udp::Builder::new(
        src_ip,
        match dst_ip {
            IpAddr::V4(addr) => addr,
            IpAddr::V6(_) => todo!("IPv6 support"),
        },
        ports::FIXED_DNS_CLIENT,
        53,
        payload,
    );

    let ef = ethernet::Frame {
        header: ethernet::FrameHeader {
            dst_mac,
            src_mac,
            ethertype: EtherType::Ipv4,
        },
        payload: udp_payload.build(),
    };

    let mut packet = ef.to_bytes();
    while packet.len() < 64 {
        packet.push(0);
    }

    ipc::publish("nic/send", &packet).map_err(|_| SendError)?;
    Ok(())
}

struct SendError;
