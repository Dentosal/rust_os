use x86_64::{PhysAddr, VirtAddr};

use super::prelude::*;

pub trait EitherAddr {
    fn as_u64(self) -> u64;
    fn from_u64(_: u64) -> Self;
}
impl EitherAddr for PhysAddr {
    fn as_u64(self) -> u64 {
        self.as_u64()
    }
    fn from_u64(addr: u64) -> Self {
        PhysAddr::new(addr)
    }
}
impl EitherAddr for VirtAddr {
    fn as_u64(self) -> u64 {
        self.as_u64()
    }
    fn from_u64(addr: u64) -> Self {
        VirtAddr::new(addr)
    }
}

/// Align up or down to page size
pub fn page_align<T: EitherAddr>(address: T, upwards: bool) -> T {
    T::from_u64(page_align_u64(address.as_u64(), upwards))
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
