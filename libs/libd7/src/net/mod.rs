#![allow(unreachable_code)] // TODO remove this

use alloc::vec::Vec;
use core::convert::TryFrom;
use serde::{Deserialize, Serialize};

pub use d7net;

pub mod tcp;
// pub mod udp;

pub use d7net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::ipc;
use d7net::dns;

/// Used to acknowledge a reliable message
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct SocketId(u64);
impl SocketId {
    pub fn from_u64(v: u64) -> Self {
        Self(v)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Possible errors when sending to or receiving from network
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum NetworkError {
    /// No suitable interfaces available
    NoInterfaces,
    /// No routers available
    NoRouters,
    /// A required ARP entry is misising
    NoArpEntry,
    /// No IP address configured for the interface
    NoIpAddr,
    /// Error occurred during name resolution (usually DNS error)
    NameResolution,
    /// Socket address was not valid, or did not resolve to any address
    InvalidSocketAddr,
}

pub trait ToSocketAddrs {
    type Iter: Iterator<Item = SocketAddr>;
    fn to_socket_addrs(&self) -> Result<Self::Iter, NetworkError>;
}

impl ToSocketAddrs for SocketAddr {
    type Iter = impl Iterator<Item = SocketAddr>;
    fn to_socket_addrs(&self) -> Result<Self::Iter, NetworkError> {
        Ok(core::iter::once(*self))
    }
}

impl ToSocketAddrs for (IpAddr, u16) {
    type Iter = impl Iterator<Item = SocketAddr>;
    fn to_socket_addrs(&self) -> Result<Self::Iter, NetworkError> {
        Ok(core::iter::once(SocketAddr {
            host: self.0,
            port: self.1,
        }))
    }
}

impl ToSocketAddrs for (Ipv4Addr, u16) {
    type Iter = impl Iterator<Item = SocketAddr>;
    fn to_socket_addrs(&self) -> Result<Self::Iter, NetworkError> {
        Ok(core::iter::once(SocketAddr {
            host: IpAddr::V4(self.0),
            port: self.1,
        }))
    }
}

impl ToSocketAddrs for (Ipv6Addr, u16) {
    type Iter = impl Iterator<Item = SocketAddr>;
    fn to_socket_addrs(&self) -> Result<Self::Iter, NetworkError> {
        Ok(core::iter::once(SocketAddr {
            host: IpAddr::V6(self.0),
            port: self.1,
        }))
    }
}

impl ToSocketAddrs for (&str, u16) {
    type Iter = impl Iterator<Item = SocketAddr>;

    #[auto_enums::auto_enum(Transpose, Iterator)]
    fn to_socket_addrs(&self) -> Result<Self::Iter, NetworkError> {
        let host: &str = self.0;
        let port: u16 = self.1;

        if let Ok(addr) = IpAddr::try_from(host) {
            Ok(core::iter::once(SocketAddr { host: addr, port }))
        } else {
            // Resolve address
            // TODO: ipv6 support
            let r: Result<Vec<dns::QueryResult>, dns::NxDomain> =
                match ipc::request("netd/dns/resolve", (host, dns::QueryType::A)) {
                    Ok(ok) => ok,
                    Err(syscall_error) => match syscall_error {
                        d7abi::SyscallErrorCode::ipc_delivery_target_nack => {
                            return Err(NetworkError::NameResolution);
                        },
                        other => panic!("Syscall error {:?}", other),
                    },
                };

            #[nested]
            match r {
                Ok(v) => Ok(v.into_iter().map(move |a| match a {
                    dns::QueryResult::A(addr) => SocketAddr {
                        host: IpAddr::V4(addr),
                        port,
                    },
                    _ => unreachable!("Mismatching DNS record returned"),
                })),
                Err(_) => Ok(core::iter::empty()),
            }
        }
        .transpose_ok()
    }
}

impl ToSocketAddrs for &str {
    type Iter = impl Iterator<Item = SocketAddr>;
    fn to_socket_addrs(&self) -> Result<Self::Iter, NetworkError> {
        let mut it = self.rsplit(":");
        let port = it
            .next()
            .and_then(|v| v.parse::<u16>().ok())
            .ok_or(NetworkError::InvalidSocketAddr)?;

        let host_str = it.next().ok_or(NetworkError::InvalidSocketAddr)?;

        if it.next().is_some() {
            return Err(NetworkError::InvalidSocketAddr);
        }

        // Accept form [::1]:80 and even [127.0.0.1]:80 had [example.org]:80
        let host_str = host_str
            .strip_prefix("[")
            .and_then(|s| s.strip_suffix("]"))
            .unwrap_or(host_str);

        (host_str, port).to_socket_addrs()
    }
}
