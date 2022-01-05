use alloc::vec::Vec;
use core::convert::TryFrom;
use num_enum::TryFromPrimitive;

use crate::{Ipv4Addr, MacAddr};

pub const MAGIC_COOKIE: u32 = 0x63825363;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum MsgType {
    QUERY = 1,
    REPLY = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum Op {
    DISCOVER = 1,
    OFFER = 2,
    REQUEST = 3,
    DECLINE = 4,
    ACK = 5,
    NAK = 6,
    RELEASE = 7,
    INFORM = 8,
    FORCERENEW = 9,
    LEASEQUERY = 10,
    LEASEUNASSIGNED = 11,
    LEASEUNKNOWN = 12,
    LEASEACTIVE = 13,
    BULKLEASEQUERY = 14,
    LEASEQUERYDONE = 15,
    ACTIVELEASEQUERY = 16,
    LEASEQUERYSTATUS = 17,
    TLS = 18,
}

#[derive(Debug, Clone)]
pub struct Payload {
    pub op: MsgType,
    pub xid: u32,
    pub client_ip: Ipv4Addr,
    pub your_ip: Ipv4Addr,
    pub server_ip: Ipv4Addr,
    pub gateway_ip: Ipv4Addr,
    pub mac_addr: MacAddr,
    pub options: Vec<DhcpOption>,
}
impl Payload {
    pub fn discover(xid: u32, mac_addr: MacAddr) -> Self {
        Self {
            op: MsgType::QUERY,
            xid,
            client_ip: Ipv4Addr::ZERO,
            your_ip: Ipv4Addr::ZERO,
            server_ip: Ipv4Addr::ZERO,
            gateway_ip: Ipv4Addr::ZERO,
            mac_addr,
            options: vec![DhcpOption::Op(Op::DISCOVER)],
        }
    }

    pub fn request(xid: u32, mac_addr: MacAddr, client_ip: Ipv4Addr, server_ip: Ipv4Addr) -> Self {
        Self {
            op: MsgType::QUERY,
            xid,
            client_ip,
            your_ip: Ipv4Addr::ZERO,
            server_ip,
            gateway_ip: Ipv4Addr::ZERO,
            mac_addr,
            options: vec![
                DhcpOption::Op(Op::REQUEST),
                DhcpOption::RequestedAddress(client_ip),
                DhcpOption::ServerId(server_ip),
                DhcpOption::ParamReqList(vec![
                    ParamReq::SubnetMask,
                    ParamReq::Router,
                    ParamReq::DNSServer,
                ]),
            ],
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let op = MsgType::try_from(bytes[0]).expect("Unknown DHCP MsgType");
        assert!(bytes[1] == 0x01, "HTYPE != MAC");
        assert!(bytes[2] == 0x06, "HLEN != 6");
        assert!(bytes[3] == 0x00, "HOPS != 0");
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&bytes[4..8]);
        let xid = u32::from_be_bytes(buf);
        buf.copy_from_slice(&bytes[12..16]);
        let client_ip = Ipv4Addr::from_bytes(&buf);
        buf.copy_from_slice(&bytes[16..20]);
        let your_ip = Ipv4Addr::from_bytes(&buf);
        buf.copy_from_slice(&bytes[20..24]);
        let server_ip = Ipv4Addr::from_bytes(&buf);
        buf.copy_from_slice(&bytes[24..28]);
        let gateway_ip = Ipv4Addr::from_bytes(&buf);
        let mac_addr = MacAddr::from_bytes(&bytes[28..34]);
        let body_i = 44 + 64 + 128;
        buf.copy_from_slice(&bytes[body_i..body_i + 4]);
        let magic = u32::from_be_bytes(buf);
        assert_eq!(magic, MAGIC_COOKIE);

        let mut options = Vec::new();
        let mut i = body_i + 4;
        loop {
            let (opt, len) = DhcpOption::from_bytes(&bytes[i..]);
            if opt == DhcpOption::End {
                break;
            }
            options.push(opt);
            i += len;
        }

        Self {
            op,
            xid,
            client_ip,
            your_ip,
            server_ip,
            gateway_ip,
            mac_addr,
            options,
        }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        let mut result = vec![
            self.op as u8, // OP
            0x01,          // HTYPE: MAC
            0x06,          // HLEN: 6
            0x00,          // HOPS: 0
        ];
        result.extend(&u32::to_be_bytes(self.xid));
        result.extend(&[
            0x00, 0x00, // SECS
            0x00, 0x00, // FLAGS
        ]);
        result.extend(&self.client_ip.0);
        result.extend(&self.your_ip.0);
        result.extend(&self.server_ip.0);
        result.extend(&self.gateway_ip.0);
        // MAC address, padded to 16 bytes
        result.extend(&self.mac_addr.0);
        result.extend(&[0u8; 10]);
        // Server name, unused
        result.extend(&[0u8; 64]);
        // Boot file name, unused
        result.extend(&[0u8; 128]);
        // DHCP magic cookie
        result.extend(&u32::to_be_bytes(MAGIC_COOKIE));
        // DHCP options
        for opt in self.options {
            result.extend(&opt.to_bytes());
        }
        // DHCP end options
        result.push(0xff);

        result
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DhcpOption {
    Pad,
    Op(Op),
    SubnetMask(Ipv4Addr),
    Routers(Vec<Ipv4Addr>),
    DnsServers(Vec<Ipv4Addr>),
    LeaseTime { seconds: u32 },
    RequestedAddress(Ipv4Addr),
    ServerId(Ipv4Addr),
    ParamReqList(Vec<ParamReq>),
    End,
    Unknown,
}
impl DhcpOption {
    pub fn from_bytes(bytes: &[u8]) -> (Self, usize) {
        let code = bytes[0];

        if code == 0x00 {
            return (Self::Pad, 1);
        } else if code == 0xff {
            return (Self::End, 1);
        }

        let length = bytes[1] as usize;

        let item = match code {
            0x01 => {
                assert!(length == 4);
                Self::SubnetMask(Ipv4Addr::from_bytes(&bytes[2..6]))
            },
            0x03 => {
                assert!(length % 4 == 0);
                let mut items = Vec::new();
                for i in 0..(length / 4) {
                    items.push(Ipv4Addr::from_bytes(&bytes[2 + i * 4..2 + i * 4 + 4]));
                }
                Self::Routers(items)
            },
            0x06 => {
                assert!(length % 4 == 0);
                let mut items = Vec::new();
                for i in 0..(length / 4) {
                    items.push(Ipv4Addr::from_bytes(&bytes[2 + i * 4..2 + i * 4 + 4]));
                }
                Self::DnsServers(items)
            },
            0x33 => {
                assert!(length == 4);
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&bytes[2..6]);
                Self::LeaseTime {
                    seconds: u32::from_be_bytes(buf),
                }
            },
            0x35 => {
                assert!(length == 1);
                Self::Op(Op::try_from(bytes[2]).expect("Unknown DHCP op"))
            },
            0x36 => {
                assert!(length == 4);
                Self::ServerId(Ipv4Addr::from_bytes(&bytes[2..6]))
            },
            0x37 => {
                let items = bytes[2..2 + length]
                    .iter()
                    .map(|b| ParamReq::try_from(*b).expect("Uknown paramreq"))
                    .collect();
                Self::ParamReqList(items)
            },
            _other => {
                // log::warn!("Unknown DHCP option {:#02x}", other);
                Self::Unknown
            },
        };

        (item, 2 + length)
    }

    pub fn to_bytes(self) -> Vec<u8> {
        match self {
            Self::Pad => vec![0x00],
            Self::End => vec![0xff],
            Self::Op(op) => vec![0x35, 0x01, op as u8],
            Self::RequestedAddress(addr) => {
                let mut result = vec![0x32, 0x04];
                result.extend(&addr.0);
                result
            },
            Self::ServerId(id) => {
                let mut result = vec![0x36, 0x04];
                result.extend(&id.0);
                result
            },
            Self::ParamReqList(items) => {
                let mut result = vec![0x37, items.len() as u8];
                result.extend(items.into_iter().map(|v| v as u8));
                result
            },
            other => todo!("Serialization unsupported for {:?}", other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum ParamReq {
    SubnetMask = 0x01,
    Router = 0x03,
    DNSServer = 0x06,
    DomainName = 0x0f,
}
