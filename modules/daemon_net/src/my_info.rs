use libd7::net::d7net::{Ipv4Addr, Ipv6Addr, MacAddr};

#[derive(Debug, Clone, Copy)]
pub struct MyInfo {
    pub mac: MacAddr,
    pub ipv4: Option<Ipv4Addr>,
    pub ipv6: Option<Ipv6Addr>,
}
