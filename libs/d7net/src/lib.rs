//! Panics on invalid data

#![cfg_attr(not(test), no_std)]
#![feature(default_free_fn, duration_constants)]
// Lints
#![allow(incomplete_features)]

#[macro_use]
extern crate alloc;

mod checksum;
mod ethertype;
mod ip_addr;
mod ip_protocol;
mod mac;

pub mod arp;
pub mod dhcp;
pub mod dns;
pub mod ethernet;
pub mod ipv4;
pub mod tcp;
pub mod udp;

pub mod builder;

pub use self::ethertype::EtherType;
pub use self::ip_addr::*;
pub use self::ip_protocol::IpProtocol;
pub use self::mac::MacAddr;
