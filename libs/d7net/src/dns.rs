//! A minimal subset of DNS for queries

use alloc::string::String;
use alloc::vec::Vec;
use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};

use crate::{Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum MsgType {
    QUERY = 0,
    REPLY = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum Opcode {
    Query = 0,
    IQuery = 1,
    Status = 2,
    Notify = 4,
    Update = 5,
    Stateful = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum RCode {
    Success = 0,
    FormatError = 1,
    ServerError = 2,
    NxDomain = 3,
    NotSupported = 4,
    Refused = 5,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    TryFromPrimitive,
    Serialize,
    Deserialize,
)]
#[repr(u16)]
pub enum QueryType {
    A = 0x0001,
    NS = 0x0002,
    CNAME = 0x0005,
    MX = 0x000f,
    AAAA = 0x001c,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryResult {
    A(Ipv4Addr),
    AAAA(Ipv6Addr),
    NS(String),
    CNAME(String),
    MX { priority: u16, domain: String },
}
impl QueryResult {
    pub fn query(&self) -> QueryType {
        match self {
            Self::A(_) => QueryType::A,
            Self::AAAA(_) => QueryType::AAAA,
            Self::NS(_) => QueryType::NS,
            Self::CNAME(_) => QueryType::CNAME,
            Self::MX { .. } => QueryType::MX,
        }
    }
}

pub fn make_question(reg_id: u16, domain: &str, qtype: QueryType) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend(&reg_id.to_be_bytes());
    result.extend(&(1u16 << 8).to_be_bytes()); // Recursive query
    result.extend(&1u16.to_be_bytes());
    result.extend(&0u16.to_be_bytes());
    result.extend(&0u16.to_be_bytes());
    result.extend(&0u16.to_be_bytes());
    for seg in domain.split('.') {
        assert!(!seg.is_empty());
        assert!(seg.is_ascii());
        assert!(seg.len() < 64);
        result.push(seg.len() as u8);
        result.extend(seg.bytes());
    }
    result.push(0);
    result.extend(&(qtype as u16).to_be_bytes());
    result.extend(&1u16.to_be_bytes()); // Internet record
    result
}

fn read_name(mut index: usize, data: &[u8]) -> (String, usize) {
    let mut name = String::new();
    let mut non_compressed_len = 0;
    let mut in_compressed = false;
    loop {
        let compressed = data[index] & 0xc0 != 0;
        index = if compressed {
            let start = u16::from_be_bytes([data[index], data[index + 1]]);
            if !in_compressed {
                in_compressed = true;
                non_compressed_len += 2;
            }
            (start & 0x3fff) as usize
        } else {
            index
        };

        let seg_len = data[index] as usize;
        assert!(seg_len & !0x3f == 0);

        index += 1;
        if !in_compressed {
            non_compressed_len += 1;
        }

        if seg_len == 0 {
            break;
        }

        if !name.is_empty() {
            name.push('.');
        }
        for offset in 0..seg_len {
            name.push(data[index + offset] as char);
        }

        index += seg_len;
        if !in_compressed {
            non_compressed_len += seg_len;
        }
    }

    (name, non_compressed_len)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TTL {
    pub seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub req_id: u16,
    pub query: (String, QueryType),
    pub records: Result<Vec<(String, TTL, QueryResult)>, NxDomain>,
}

/// Marker type for "no such domain" error
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NxDomain;

pub fn parse_reply(data: &[u8]) -> Result<Reply, &str> {
    let mut buf = [0u8; 2];

    buf.copy_from_slice(&data[0..2]);
    let req_id = u16::from_be_bytes(buf);

    buf.copy_from_slice(&data[2..4]);
    let flags = u16::from_be_bytes(buf);

    if flags & (1 << 15) == 0 {
        return Err("Not a reply");
    }

    if flags & (1 << 9) != 0 {
        return Err("Truncated");
    }

    if flags & (1 << 7) == 0 {
        return Err("Could not recurse");
    }

    let Ok(rcode) = RCode::try_from_primitive((flags as u8) & 0x0f) else {
        return Err("Invalid rcode");
    };

    match rcode {
        RCode::Success => {
            buf.copy_from_slice(&data[4..6]);
            let count_qd = u16::from_be_bytes(buf);
            buf.copy_from_slice(&data[6..8]);
            let count_an = u16::from_be_bytes(buf);
            buf.copy_from_slice(&data[8..10]);
            let _count_ns = u16::from_be_bytes(buf);
            buf.copy_from_slice(&data[10..12]);
            let _count_ar = u16::from_be_bytes(buf);

            let mut i = 12;

            // Parse and skip query section
            assert_eq!(count_qd, 1);
            let (query_name, size) = read_name(i, data);
            i += size;
            buf.copy_from_slice(&data[i..i + 2]);
            let qtype = u16::from_be_bytes(buf);
            let Ok(query_type) = QueryType::try_from_primitive(qtype) else {
                panic!("Unknown query type reply {:?}", qtype);
            };
            i += 4; // Includes rest of the fields

            // Parse answer section
            let mut records = Vec::new();
            for _ in 0..count_an {
                let (name, size) = read_name(i, data);
                i += size;

                buf.copy_from_slice(&data[i..i + 2]);
                let qtype = u16::from_be_bytes(buf);
                i += 2;

                buf.copy_from_slice(&data[i..i + 2]);
                let class = u16::from_be_bytes(buf);
                i += 2;

                let mut buf4 = [0u8; 4];
                buf4.copy_from_slice(&data[i..i + 4]);
                let ttl = TTL {
                    seconds: u32::from_be_bytes(buf4),
                };
                i += 4;

                buf.copy_from_slice(&data[i..i + 2]);
                let payload_len = u16::from_be_bytes(buf) as usize;
                i += 2;
                let payload_start = i;
                let payload = &data[i..i + payload_len];
                i += payload_len;

                if class != 1 {
                    log::warn!("Ignoring unknown class {}", class);
                    continue;
                }

                let Ok(qtype) = QueryType::try_from_primitive(qtype) else {
                    log::warn!("Unknown query type {:?}", qtype);
                    continue;
                };

                let payload = match qtype {
                    QueryType::A => {
                        assert_eq!(payload_len, 4, "Invalid A record payload size");
                        QueryResult::A(Ipv4Addr::from_bytes(payload))
                    },
                    QueryType::AAAA => {
                        assert_eq!(payload_len, 16, "Invalid A record payload size");
                        QueryResult::AAAA(Ipv6Addr::from_bytes(payload))
                    },
                    QueryType::MX => {
                        buf.copy_from_slice(&payload[..2]);
                        let priority = u16::from_be_bytes(buf);
                        let (domain, sz) = read_name(payload_start + 2, data);
                        debug_assert_eq!(payload.len() - 2, sz);
                        QueryResult::MX { priority, domain }
                    },
                    other => {
                        log::warn!("Unsupported query response type {:?}", other);
                        continue;
                    },
                };

                records.push((name, ttl, payload));
            }

            // Ignore ns and ar

            Ok(Reply {
                req_id,
                query: (query_name, query_type),
                records: Ok(records),
            })
        },
        RCode::FormatError => Err("Format error"),
        RCode::ServerError => Err("Server error"),
        RCode::NxDomain => {
            buf.copy_from_slice(&data[4..6]);
            let count_qd = u16::from_be_bytes(buf);

            let mut i = 12;

            // Parse and skip query section
            assert_eq!(count_qd, 1);
            let (query_name, size) = read_name(i, data);
            i += size;
            buf.copy_from_slice(&data[i..i + 2]);
            let qtype = u16::from_be_bytes(buf);
            let Ok(query_type) = QueryType::try_from_primitive(qtype) else {
                panic!("Unknown query type reply {:?}", qtype);
            };

            Ok(Reply {
                req_id,
                query: (query_name, query_type),
                records: Err(NxDomain),
            })
        },
        RCode::NotSupported => Err("Server does not support this"),
        RCode::Refused => Err("Server refused"),
    }
}
