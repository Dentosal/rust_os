//! Panics on invalid data

#![cfg_attr(not(test), no_std)]
// Lints
#![allow(incomplete_features)]
// Features
#![feature(alloc_prelude)]
#![feature(const_generics)]
// Temp
#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_mut)]

#[macro_use]
extern crate alloc;

mod checksum;
mod ethertype;
mod ip_addr;
mod ip_protocol;
mod mac;

pub mod arp;
pub mod ethernet;
pub mod ipv4;
pub mod ipv6;
pub mod ipv_either;
pub mod tcp;

pub use self::ethertype::EtherType;
pub use self::ip_addr::*;
pub use self::ip_protocol::IpProtocol;
pub use self::mac::MacAddr;
