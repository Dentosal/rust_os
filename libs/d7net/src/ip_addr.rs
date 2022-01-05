use core::convert::TryFrom;
use core::fmt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct Ipv4Addr(pub [u8; 4]);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct Ipv6Addr(pub [u8; 16]);

impl Ipv4Addr {
    pub const ZERO: Self = Self([0, 0, 0, 0]);
    pub const LOCALHOST: Self = Self([127, 0, 0, 1]);
    pub const BROADCAST: Self = Self([255, 255, 255, 255]);

    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() == 4);
        let mut data = [0; 4];
        data.copy_from_slice(bytes);
        Ipv4Addr(data)
    }
}

impl Ipv6Addr {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() == 16);
        let mut data = [0; 16];
        data.copy_from_slice(bytes);
        Ipv6Addr(data)
    }
}

impl fmt::Debug for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ipv4Addr({})", self)
    }
}

impl fmt::Debug for Ipv6Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ipv6Addr({})", self)
    }
}

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut it = self.0.iter().peekable();
        while let Some(p) = it.next() {
            write!(f, "{}", p)?;
            if it.peek().is_some() {
                write!(f, ".")?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for Ipv6Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut it = self.0.iter().peekable();
        while let Some(p) = it.next() {
            write!(f, "{:02x}", p)?;
            if it.peek().is_some() {
                write!(f, ":")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidIpv4Addr;

impl TryFrom<&str> for Ipv4Addr {
    type Error = InvalidIpv4Addr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut buffer = [0u8; 4];
        let mut s = value.split(".");
        for v in &mut buffer {
            let a = s.next().ok_or(InvalidIpv4Addr)?;
            *v = a.parse::<u8>().map_err(|_| InvalidIpv4Addr)?;
        }
        if s.next().is_some() {
            Err(InvalidIpv4Addr)
        } else {
            Ok(Self(buffer))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum IpAddr {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}
impl IpAddr {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        match bytes.len() {
            4 => Self::V4(Ipv4Addr::from_bytes(bytes)),
            16 => Self::V6(Ipv6Addr::from_bytes(bytes)),
            _ => panic!("Invalid ip address byte count"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidIpAddr;

impl TryFrom<&str> for IpAddr {
    type Error = InvalidIpAddr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // TODO: Ipv6 parsing
        Ok(Self::V4(
            Ipv4Addr::try_from(value).map_err(|_| InvalidIpAddr)?,
        ))
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct SocketAddr {
    pub host: IpAddr,
    pub port: u16,
}

impl SocketAddr {
    pub const ZERO: Self = Self {
        host: IpAddr::V4(Ipv4Addr::ZERO),
        port: 0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidSocketAddr;

impl TryFrom<&str> for SocketAddr {
    type Error = InvalidSocketAddr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut it = value.rsplit(":");
        let port = it
            .next()
            .and_then(|v| v.parse::<u16>().ok())
            .ok_or(InvalidSocketAddr)?;

        let host_str = it.next().ok_or(InvalidSocketAddr)?;

        if it.next().is_some() {
            return Err(InvalidSocketAddr);
        }

        // Accept form [::1]:80 and even [127.0.0.1]:80
        let host_str = host_str
            .strip_prefix("[")
            .and_then(|s| s.strip_suffix("]"))
            .unwrap_or(host_str);

        Self::try_from((host_str, port))
    }
}

impl TryFrom<(&str, u16)> for SocketAddr {
    type Error = InvalidSocketAddr;

    fn try_from((host_str, port): (&str, u16)) -> Result<Self, Self::Error> {
        let host = IpAddr::try_from(host_str).map_err(|_| InvalidSocketAddr)?;
        Ok(Self { host, port })
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;

    #[test]
    fn parse_addr_ipv4() {
        assert_eq!("1.2.3.4".try_into(), Ok(Ipv4Addr([1, 2, 3, 4])));
        assert_eq!("0.0.0.0".try_into(), Ok(Ipv4Addr([0, 0, 0, 0])));
        assert_eq!(
            "255.255.255.255".try_into(),
            Ok(Ipv4Addr([255, 255, 255, 255]))
        );
        for case in [
            "1.2.3.4.",
            "1.2.3.",
            "1.2.3.4.",
            "256.2.3.1",
            "random text data",
            "::1",
            "...",
            "1 2 3 4",
        ] {
            let ip: Result<Ipv4Addr, _> = case.try_into();
            assert_eq!(ip, Err(InvalidIpv4Addr));
        }
    }

    #[test]
    fn parse_socket_addr() {
        assert_eq!(
            "0.0.0.0:0".try_into(),
            Ok(SocketAddr {
                host: IpAddr::V4(Ipv4Addr([0, 0, 0, 0])),
                port: 0,
            })
        );
        assert_eq!(
            "255.255.255.255:65535".try_into(),
            Ok(SocketAddr {
                host: IpAddr::V4(Ipv4Addr([255, 255, 255, 255])),
                port: 65535,
            })
        );

        for case in [
            "1.2.3.4",
            "1.2.3.4.",
            "1.2.3.4:",
            "1.2.3:12",
            "1.2.3.4:123456",
            "random text data",
            "::1",
            "...",
            "1 2 3 4",
            "1 2 3 4 5",
        ] {
            let ip: Result<SocketAddr, _> = case.try_into();
            assert_eq!(ip, Err(InvalidSocketAddr));
        }
    }
}
