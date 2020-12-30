use alloc::prelude::v1::*;
use core::fmt;
use serde::{Deserialize, Serialize};

const V4_IN_V6_PREFIX: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct Ipv4Addr(pub [u8; 4]);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct Ipv6Addr(pub [u8; 16]);

impl Ipv4Addr {
    pub const ZERO: Self = Self([0, 0, 0, 0]);
    pub const LOOPBACK: Self = Self([127, 0, 0, 1]);

    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() == 4);
        let mut data = [0; 4];
        data.copy_from_slice(bytes);
        Ipv4Addr(data)
    }

    pub fn from_str(s: &str) -> Option<Self> {
        if s.chars().filter(|&c| c == '.').count() != 3 {
            return None;
        }
        let mut bytes = [0; 4];
        for (i, item) in s.split('.').enumerate() {
            bytes[i] = item.parse::<u8>().ok()?;
        }
        Some(Self(bytes))
    }

    pub fn as_int(self) -> u32 {
        u32::from_be_bytes(self.0)
    }

    /// Convert IPv4 address to "IPv4-mapped IPv6 address"
    /// https://tools.ietf.org/html/rfc4291#section-2.5.5.2
    ///
    /// Also converts unspecified (zero) and loopback addresses correctly.
    pub fn to_ipv6(self) -> Ipv6Addr {
        if self == Self::ZERO {
            return Ipv6Addr::ZERO;
        } else if self == Self::LOOPBACK {
            return Ipv6Addr::LOOPBACK;
        } else {
            let mut data = [0; 16];
            data[..12].copy_from_slice(&V4_IN_V6_PREFIX);
            data[12..].copy_from_slice(&self.0);
            Ipv6Addr(data)
        }
    }

    pub fn to_generic(self) -> IpAddr {
        IpAddr::V4(self)
    }
}

impl Ipv6Addr {
    pub const ZERO: Self = Self([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    pub const LOOPBACK: Self = Self([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() == 16);
        let mut data = [0; 16];
        data.copy_from_slice(bytes);
        Ipv6Addr(data)
    }

    pub fn from_str(s: &str) -> Option<Self> {
        let mut groups: Vec<_> = s.split(':').collect();
        if groups.len() < 2 {
            return None;
        }

        let omit_s = groups.first().unwrap().is_empty();
        let omit_e = groups.last().unwrap().is_empty();
        if omit_s {
            groups.remove(0);
        }
        if omit_e {
            groups.pop();
            groups.pop();
        }

        // From the end
        let mut result = [0u8; 16];
        let mut end = 16;
        if !omit_e {
            while let Some(h) = groups.pop() {
                if end == 0 {
                    return None;
                }
                if h.is_empty() {
                    // "::"
                    break;
                }
                let q = u16::from_str_radix(h, 16).ok()?;
                result[end - 2] = (q >> 8) as u8;
                result[end - 1] = q as u8;
                end -= 2;
            }
        }

        if omit_s {
            if !groups.is_empty() {
                return None;
            }
        } else {
            // From the start
            for (i, h) in groups.into_iter().enumerate() {
                if h.is_empty() || i * 2 >= end {
                    // Multiple "::" or too long
                    return None;
                }
                let q = u16::from_str_radix(h, 16).ok()?;
                result[i * 2] = (q >> 8) as u8;
                result[i * 2 + 1] = q as u8;
            }
        }
        Some(Self(result))
    }

    pub fn as_int(self) -> u128 {
        u128::from_be_bytes(self.0)
    }

    /// 16-bit groups
    pub fn quibbles(self) -> [u16; 8] {
        let mut result = [0; 8];
        for i in 0..result.len() {
            result[i] = u16::from_be_bytes([self.0[i * 2], self.0[i * 2 + 1]]);
        }
        result
    }

    /// If this is an "IPv4-mapped IPv6 address", convert it to an IPv4 address.
    /// https://tools.ietf.org/html/rfc4291#section-2.5.5.2
    ///
    /// Also converts unspecified (zero) and loopback addresses
    pub fn to_ipv4(self) -> Option<Ipv4Addr> {
        if self.0[..12] == V4_IN_V6_PREFIX {
            Some(Ipv4Addr::from_bytes(&self.0[12..]))
        } else if self == Self::ZERO {
            Some(Ipv4Addr::ZERO)
        } else if self == Self::LOOPBACK {
            Some(Ipv4Addr::LOOPBACK)
        } else {
            None
        }
    }

    pub fn to_generic(self) -> IpAddr {
        if let Some(v4) = self.to_ipv4() {
            IpAddr::V4(v4)
        } else {
            IpAddr::V6(self)
        }
    }
}

impl fmt::Debug for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<_> = self.0.iter().map(|c| format!("{}", c)).collect();
        write!(f, "{}", parts.join("."))
    }
}

impl fmt::Debug for Ipv6Addr {
    /// https://en.wikipedia.org/wiki/IPv6_address#Recommended_representation_as_text
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let qs = self.quibbles();
        // Find most consecutive zeros
        let mut zeros_i: usize = 0; // index
        let mut zeros_n: usize = 0; // number of zeros
        let mut i = 0;
        while i < qs.len() {
            let mut offset = 0;
            while i + offset < qs.len() && qs[i + offset] == 0 {
                offset += 1;
            }
            if offset >= zeros_n {
                zeros_i = i;
                zeros_n = offset;
            }
            i += offset + 1;
        }
        for (i, q) in qs.iter().enumerate() {
            if zeros_n >= 2 {
                // Omit fields only if there are at least two of them
                if i == zeros_i {
                    write!(f, ":")?;
                    if i == 0 || i == 7 {
                        write!(f, ":")?;
                    }
                    continue;
                } else if zeros_i < i && i < zeros_i + zeros_n {
                    continue;
                }
            }
            write!(f, "{:x}", q)?;
            if i != 7 {
                write!(f, ":")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum IpAddr {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}
impl IpAddr {
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        match bytes.len() {
            4 => Self::V4(Ipv4Addr::from_bytes(bytes)),
            16 => Self::V6(Ipv6Addr::from_bytes(bytes)),
            _ => panic!("Invalid ip address byte count"),
        }
    }

    pub fn to_ipv6(self) -> Ipv6Addr {
        match self {
            Self::V4(ip) => ip.to_ipv6(),
            Self::V6(ip) => ip,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[rustfmt::skip]
    fn test_ipv4_parse_and_format() {
        assert_eq!(&format!("{:?}", Ipv4Addr::ZERO), "0.0.0.0");
        assert_eq!(&format!("{:?}", Ipv4Addr::LOOPBACK), "127.0.0.1");

        let addr0 = Ipv4Addr::from_str("0.0.0.0").unwrap();
        let addr1 = Ipv4Addr::from_str("127.0.0.1").unwrap();
        let addr2 = Ipv4Addr::from_str("1.2.3.4").unwrap();
        let addr3 = Ipv4Addr::from_str("12.34.56.78").unwrap();
        let addr4 = Ipv4Addr::from_str("255.0.255.0").unwrap();
        let addr5 = Ipv4Addr::from_str("002.020.200.000").unwrap();

        assert_eq!(&format!("{:?}", addr0), "0.0.0.0");
        assert_eq!(&format!("{:?}", addr1), "127.0.0.1");
        assert_eq!(&format!("{:?}", addr2), "1.2.3.4");
        assert_eq!(&format!("{:?}", addr3), "12.34.56.78");
        assert_eq!(&format!("{:?}", addr4), "255.0.255.0");
        assert_eq!(&format!("{:?}", addr5), "2.20.200.0");

        assert_eq!(Ipv4Addr::from_str("1.2.3"), None);
        assert_eq!(Ipv4Addr::from_str("1.2.3.4.5"), None);
        assert_eq!(Ipv4Addr::from_str(""), None);
        assert_eq!(Ipv4Addr::from_str("256.0.0.0"), None);
        assert_eq!(Ipv4Addr::from_str("0.0.0.256"), None);
        assert_eq!(Ipv4Addr::from_str("1"), None);
        assert_eq!(Ipv4Addr::from_str("1.2"), None);
        assert_eq!(Ipv4Addr::from_str("1..2"), None);
        assert_eq!(Ipv4Addr::from_str("1...2"), None);

        // Test all 1-bit-on and 2-bits-on patterns and inverted
        for bit0 in 0..32 {
            let bits0 = 1u32 << bit0;
            let addr = Ipv4Addr::from_bytes(&bits0.to_be_bytes());
            assert_eq!(Some(addr), Ipv4Addr::from_str(&format!("{:?}", addr)));
            let addr_inv = Ipv4Addr::from_bytes(&(!bits0).to_be_bytes());
            assert_eq!(Some(addr_inv), Ipv4Addr::from_str(&format!("{:?}", addr_inv)));
            for bit1 in 0..bit0 {
                let bits1 = bits0 | (1u32 << bit1);
                let addr = Ipv4Addr::from_bytes(&bits1.to_be_bytes());
                assert_eq!(Some(addr), Ipv4Addr::from_str(&format!("{:?}", addr)));
                let addr_inv = Ipv4Addr::from_bytes(&(!bits1).to_be_bytes());
                assert_eq!(Some(addr_inv), Ipv4Addr::from_str(&format!("{:?}", addr_inv)));
            }
        }
    }

    #[test]
    #[rustfmt::skip]
    fn test_ipv6_parse_and_format() {
        assert_eq!(&format!("{:?}", Ipv6Addr::ZERO), "::");
        assert_eq!(&format!("{:?}", Ipv6Addr::LOOPBACK), "::1");

        let addr0 = Ipv6Addr::from_str("::").unwrap();
        let addr1 = Ipv6Addr::from_str("::1").unwrap();
        let addr2 = Ipv6Addr::from_str("2001:0db8:85a3:0000:0000:8a2e:0370:7334").unwrap();
        let addr3 = Ipv6Addr::from_str("0000:0db8:85a3:0000:0000:8a2e:0370:7334").unwrap();
        let addr4 = Ipv6Addr::from_str("0000:0db8:85a3:0000:0000:8a2e:0370:0000").unwrap();
        let addr5 = Ipv6Addr::from_str("0001:0db8:85a3:0000:abcd:8a2e:0370:0000").unwrap();
        let addr6 = Ipv6Addr::from_str("1234:5678:9abc:def0:1234:5678:9abc:def0").unwrap();
        let addr7 = Ipv6Addr::from_str("1::").unwrap();
        let addr8 = Ipv6Addr::from_str("1::1").unwrap();

        assert_eq!(&format!("{:?}", addr0), "::");
        assert_eq!(&format!("{:?}", addr1), "::1");
        assert_eq!(&format!("{:?}", addr2), "2001:db8:85a3::8a2e:370:7334");
        assert_eq!(&format!("{:?}", addr3), "0:db8:85a3::8a2e:370:7334");
        assert_eq!(&format!("{:?}", addr4), "0:db8:85a3::8a2e:370:0");
        assert_eq!(&format!("{:?}", addr5), "1:db8:85a3:0:abcd:8a2e:370:0");
        assert_eq!(&format!("{:?}", addr6), "1234:5678:9abc:def0:1234:5678:9abc:def0");
        assert_eq!(&format!("{:?}", addr7), "1::");
        assert_eq!(&format!("{:?}", addr8), "1::1");

        assert_eq!(Ipv6Addr::from_str("1234:5678:9abc:def0:1234:5678:9abc:def0:ffff"), None);
        assert_eq!(Ipv6Addr::from_str("1234:5678:9abc:def0:1234:5678:9abc:def0:ffff:ffff"), None);
        assert_eq!(Ipv6Addr::from_str(""), None);
        assert_eq!(Ipv6Addr::from_str(":::"), None);
        assert_eq!(Ipv6Addr::from_str("1::2::3"), None);
        assert_eq!(Ipv6Addr::from_str("1::2::"), None);
        assert_eq!(Ipv6Addr::from_str("::2::3"), None);
        assert_eq!(Ipv6Addr::from_str("::2::"), None);

        // Test all 1-bit-on and 2-bits-on patterns and inverted
        for bit0 in 0..128 {
            let bits0 = 1u128 << bit0;
            let addr = Ipv6Addr::from_bytes(&bits0.to_be_bytes());
            assert_eq!(Some(addr), Ipv6Addr::from_str(&format!("{:?}", addr)));
            let addr_inv = Ipv6Addr::from_bytes(&(!bits0).to_be_bytes());
            assert_eq!(Some(addr_inv), Ipv6Addr::from_str(&format!("{:?}", addr_inv)));
            for bit1 in 0..bit0 {
                let bits1 = bits0 | (1u128 << bit1);
                let addr = Ipv6Addr::from_bytes(&bits1.to_be_bytes());
                assert_eq!(Some(addr), Ipv6Addr::from_str(&format!("{:?}", addr)));
                let addr_inv = Ipv6Addr::from_bytes(&(!bits1).to_be_bytes());
                assert_eq!(Some(addr_inv), Ipv6Addr::from_str(&format!("{:?}", addr_inv)));
            }
        }
    }

    #[test]
    fn test_convert_versions() {
        let a4 = Ipv4Addr::from_str("1.2.3.4").unwrap();
        let a6 = Ipv6Addr::from_str("::ffff:102:304").unwrap();
        assert_eq!(a4.to_ipv6(), a6);

        // Test all 1-bit-on and 2-bits-on patterns and inverted
        for bit0 in 0..32 {
            let bits0 = 1u32 << bit0;
            let addr = Ipv4Addr::from_bytes(&bits0.to_be_bytes());
            assert_eq!(Some(addr), addr.to_ipv6().to_ipv4());
            let addr_inv = Ipv4Addr::from_bytes(&(!bits0).to_be_bytes());
            assert_eq!(Some(addr_inv), addr_inv.to_ipv6().to_ipv4());
            for bit1 in 0..32 {
                let bits1 = bits0 | (1u32 << bit0);
                let addr = Ipv4Addr::from_bytes(&bits1.to_be_bytes());
                assert_eq!(Some(addr), addr.to_ipv6().to_ipv4());
                let addr_inv = Ipv4Addr::from_bytes(&(!bits1).to_be_bytes());
                assert_eq!(Some(addr_inv), addr_inv.to_ipv6().to_ipv4());
            }
        }
    }
}
