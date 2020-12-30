use serde::{Deserialize, Serialize};
use spin::Mutex;

use libd7::prelude::*;

use libd7::net::d7net::{Ipv6Addr, MacAddr};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArpCache {
    map: HashMap<Ipv6Addr, MacAddr>,
}
impl ArpCache {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, ip: Ipv6Addr, mac: MacAddr) {
        self.map.insert(ip, mac);
    }

    pub fn get(&self, ip: Ipv6Addr) -> Option<MacAddr> {
        self.map.get(&ip).copied()
    }
}

lazy_static::lazy_static! {
    pub static ref ARP_CACHE: Mutex<ArpCache> = Mutex::new(ArpCache::new());
}
