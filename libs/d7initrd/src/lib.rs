// No std
#![no_std]

extern crate alloc;

use alloc::string::String;
use serde::{Deserialize, Serialize};

pub const SECTOR_SIZE: u64 = 0x200;

/// Offset in MBR: Separator between the kernel and the ramdisk
pub const MBR_POSITION_S: u16 = 0x01f6;

/// Offset in MBR: End of ramdisk (stage0 loads sectors until this)
pub const MBR_POSITION_E: u16 = 0x01fa;

pub const HEADER_MAGIC: u32 = 0xd7_ca_fe_d7;
pub const HEADER_SIZE_BYTES: usize = 16;

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
    /// Filename
    pub name: String,
    /// Size, in bytes
    pub size: u64,
    /// Offset from the start of the file list
    pub offset: u64,
}
impl FileEntry {
    pub fn size_sectors(&self) -> u64 {
        to_sectors_round_up(self.size)
    }
}
