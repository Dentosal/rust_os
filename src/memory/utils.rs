use x86_64::PhysAddr;

use super::prelude::*;

/// Align up or down to page size
pub fn page_align(address: PhysAddr, upwards: bool) -> PhysAddr {
    let address = address.as_u64();

    if address % Page::SIZE == 0 {
        PhysAddr::new(address)
    } else if upwards {
        PhysAddr::new(address + Page::SIZE - address % Page::SIZE)
    } else {
        PhysAddr::new(address - address % Page::SIZE)
    }
}
