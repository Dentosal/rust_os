//! Panics on invalid data

#![cfg_attr(not(test), no_std)]
// Lints
#![allow(incomplete_features)]
// Features
#![feature(alloc_prelude)]
#![feature(const_generics)]

#[macro_use]
extern crate alloc;

mod ethertype;
mod ip_addr;
mod ip_protocol;
mod mac;

pub mod arp;
pub mod ethernet;
pub mod ipv4;

pub use self::ethertype::EtherType;
pub use self::ip_addr::*;
pub use self::mac::MacAddr;
