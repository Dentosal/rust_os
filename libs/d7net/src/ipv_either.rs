use serde::{Serialize, Deserialize};

use super::ipv4;
use super::ipv6;

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Header {
    V4(ipv4::Header),
    V6(ipv6::Header),
}