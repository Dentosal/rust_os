#![allow(dead_code)]

// https://datatracker.ietf.org/doc/html/rfc6335#page-11

use core::ops::RangeInclusive;

use libd7::random;

pub const RANGE_SYSTEM: RangeInclusive<u16> = 0..=1023;
pub const RANGE_USER: RangeInclusive<u16> = 1024..=49151;
pub const RANGE_DYNAMIC: RangeInclusive<u16> = 49152..=65535;

pub fn random_dynamic_port() -> u16 {
    let a: [u8; 2] = random::fast_arr();
    let v = u16::from_le_bytes(a);
    let i = v % (RANGE_DYNAMIC.len() as u16);
    RANGE_DYNAMIC.start() + i
}

// Some fixed ports that are used by builtin clients
pub const FIXED_DNS_CLIENT: u16 = 54;
