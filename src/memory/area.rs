use core::ops::Range;

use super::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq)]
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

    /// Are these areas adjacent or overlapping
    pub fn can_merge(&self, other: Self) -> bool {
        let ss = self.start();
        let se = self.end();
        let os = other.start();
        let oe = other.end();

        (ss <= os && os <= se) || (ss <= oe && oe <= se)
    }

    /// Combine two adjacent areas
    pub fn merge(&self, other: Self) -> Self {
        assert!(self.can_merge(other));

        let start = self.start().min(other.start());
        let end = self.end().max(other.end());
        Self::range(start..end)
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

#[cfg(test)]
mod tests {
    use super::super::prelude::*;
    use super::PhysMemoryRange;

    #[test]
    fn test_merge() {
        let a = PhysAddr::new(10);
        let b = PhysAddr::new(20);
        let c = PhysAddr::new(15);
        let d = PhysAddr::new(20);
        let e = PhysAddr::new(25);

        let r0 = PhysMemoryRange::range(a..b);
        let r1 = PhysMemoryRange::range(c..d);
        let r2 = PhysMemoryRange::range(c..e);

        assert!(r0.can_merge(r0));
        assert!(r0.can_merge(r1));
        assert!(r0.can_merge(r2));

        assert!(r1.can_merge(r0));
        assert!(r1.can_merge(r1));
        assert!(r1.can_merge(r2));

        let m0 = r0.merge(r1);
        let m1 = r1.merge(r0);
        assert_eq!(m0, m1);
        assert_eq!(m0.start(), a);
        assert_eq!(m1.end(), d);

        let m2 = r0.merge(r2);
        let m3 = r2.merge(r0);
        assert_eq!(m2, m3);
    }
}
