use core::ops::Range;

use super::prelude::*;

#[derive(Debug, Copy, Clone)]
pub struct PhysMemoryRange {
    start: PhysAddr,
    end: PhysAddr,
}

impl PhysMemoryRange {
    pub const fn range(range: Range<PhysAddr>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }

    /// Does this entry contain a specific physical address
    pub fn contains_address(&self, ph_addr: PhysAddr) -> bool {
        self.start() <= ph_addr && ph_addr < self.end()
    }

    /// Address of the first byte this entry points to
    pub fn start(&self) -> PhysAddr {
        self.start
    }

    /// Address of the first byte after the region pointed by this entry
    pub fn end(&self) -> PhysAddr {
        self.end
    }

    /// Size of this entry, in bytes
    pub fn size_bytes(&self) -> u64 {
        (self.end() - self.start()) as u64
    }

    /// Size of this entry, in pages
    pub fn size_pages(&self) -> u64 {
        self.size_bytes() / Page::SIZE as u64
    }

    /// Are these areas adjacent?
    pub fn can_merge(&self, other: Self) -> bool {
        self.start() == other.end() || other.start() == self.end()
    }

    /// Combine two adjacent areas
    pub fn merge(&self, other: Self) -> Self {
        assert!(self.can_merge(other));

        Self::range(
            self.start().min(other.start())..PhysAddr::new(self.size_bytes() + other.size_bytes()),
        )
    }

    /// Split this entry at ph_addr, and return the lower half, if any
    pub fn below(&self, ph_addr: PhysAddr) -> Option<Self> {
        // Empty area is not allowed, so <=
        if ph_addr <= self.start() {
            None
        } else {
            Some(Self::range(self.start()..self.end().min(ph_addr)))
        }
    }

    /// Split this entry at ph_addr, and return the upper half, if any
    pub fn above(&self, ph_addr: PhysAddr) -> Option<Self> {
        // Empty area is not allowed, so <=
        if ph_addr >= self.end() {
            None
        } else {
            Some(Self::range(self.start().max(ph_addr)..self.end()))
        }
    }
}
