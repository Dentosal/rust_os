use alloc::prelude::v1::*;
use serde::{Deserialize, Serialize};

use d7net::{IpAddr, Ipv6Addr};

/// Socket creation options by protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum SocketOptions {
    /// TCP socket, can be either Ipv4 or Ipv6
    Tcp {
        /// Ipv{4,6} address
        host: IpAddr,
        /// Zero for autoselect
        port: u16,
    },
    /// UDP socket, can be either Ipv4 or Ipv6
    Udp {
        /// Ipv{4,6} address
        host: IpAddr,
        /// Zero for autoselect
        port: u16,
    },
}

/// Socket creation result
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum SocketDescriptor {
    TcpServer {
        local: (Ipv6Addr, u16),
    },
    TcpClient {
        local: (Ipv6Addr, u16),
        remote: (Ipv6Addr, u16),
    },
    Udp {
        local: (Ipv6Addr, u16),
    },
}
impl SocketDescriptor {
    pub fn topic(&self) -> String {
        match self {
            Self::TcpClient { local, remote } => format!(
                "tcp_{:x}_{}_{:x}_{}",
                local.0.as_int(),
                local.1,
                remote.0.as_int(),
                remote.1
            ),
            Self::TcpServer { local } => format!("tcp_{:x}_{}", local.0.as_int(), local.1),
            Self::Udp { local } => format!("udp_{:x}_{}", local.0.as_int(), local.1),
        }
    }
}
