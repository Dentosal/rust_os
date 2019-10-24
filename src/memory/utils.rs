use x86_64::PhysAddr;

use super::prelude::*;

/// Align up or down to page size
pub fn page_align(address: PhysAddr, upwards: bool) -> PhysAddr {
    PhysAddr::new(page_align_u64(address.as_u64(), upwards))
}

/// Align up or down to page size
pub fn page_align_u64(address: u64, upwards: bool) -> u64 {
    if address % Page::SIZE == 0 {
        address
    } else if upwards {
        address + Page::SIZE - address % Page::SIZE
    } else {
        address - address % Page::SIZE
    }
}
