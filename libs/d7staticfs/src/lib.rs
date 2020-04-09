// No std
#![no_std]
// Features
#![feature(alloc_prelude)]

extern crate alloc;

use alloc::prelude::v1::*;
use serde::{Deserialize, Serialize};

pub const SECTOR_SIZE: u64 = 0x200;

pub const MBR_POSITION: u16 = 0x01fa;
pub const HEADER_MAGIC: u32 = 0xd7_ca_fe_d7;
pub const ONLY_VERSION: u32 = 0x00_00_00_01;
pub const HEADER_SIZE_BYTES: u64 = 16;

/// Convert file byte size to number of sectors required
pub const fn to_sectors_round_up(p: u64) -> u64 {
    (p + SECTOR_SIZE - 1) / SECTOR_SIZE
}

static_assertions::const_assert_eq!(to_sectors_round_up(0), 0);
static_assertions::const_assert_eq!(to_sectors_round_up(1), 1);
static_assertions::const_assert_eq!(to_sectors_round_up(511), 1);
static_assertions::const_assert_eq!(to_sectors_round_up(512), 1);
static_assertions::const_assert_eq!(to_sectors_round_up(513), 2);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub size_bytes: u64,
}
impl FileEntry {
    pub fn size_sectors(&self) -> u64 {
        to_sectors_round_up(self.size_bytes)
    }
}
