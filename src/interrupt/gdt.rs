use core::mem::size_of;
use core::ptr;
use core::sync::atomic::{AtomicU8, Ordering};
use x86_64::structures::gdt::SegmentSelector;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::{PrivilegeLevel, VirtAddr};

pub use x86_64::structures::gdt::Descriptor;

use crate::memory::constants::GDT_ADDR;

pub const DOUBLE_FAULT_IST_INDEX: usize = 0;

/// Max size is fixed so we can have an array of these
const GDT_MAX_SIZE: usize = 8;

pub struct GdtBuilder {
    addr: VirtAddr,
    next_entry: usize,
}
impl GdtBuilder {
    unsafe fn new(addr: VirtAddr) -> Self {
        Self {
            addr,
            next_entry: 1, // first entry is the null descriptor, so it is not free
        }
    }

    pub fn add_entry(&mut self, entry: Descriptor) -> SegmentSelector {
        let base: *mut u64 = self.addr.as_mut_ptr();
        let index = self.next_entry;
        unsafe {
            match entry {
                Descriptor::UserSegment(value) => {
                    assert!(index + 1 < GDT_MAX_SIZE, "GDT full");
                    ptr::write(base.add(self.next_entry), value);
                    self.next_entry += 1;
                },
                Descriptor::SystemSegment(value_low, value_high) => {
                    assert!(index + 2 < GDT_MAX_SIZE, "GDT full");
                    ptr::write(base.add(self.next_entry), value_low);
                    ptr::write(base.add(self.next_entry + 1), value_high);
                    self.next_entry += 2;
                },
            };
        }
        SegmentSelector::new(index as u16, PrivilegeLevel::Ring0)
    }

    pub unsafe fn load(self) {
        use core::mem::size_of;
        use x86_64::instructions::tables::{lgdt, DescriptorTablePointer};

        let ptr = DescriptorTablePointer {
            base: self.addr,
            limit: (self.next_entry * size_of::<u64>() - 1) as u16,
        };

        lgdt(&ptr);
    }
}

static USED_GDTS: AtomicU8 = AtomicU8::new(0);

/// Adds to an array of immutable GDTs, one for each processor core
pub fn create_new() -> GdtBuilder {
    let index = USED_GDTS.fetch_add(1, Ordering::SeqCst);
    let new_gdt_base =
        GDT_ADDR.as_u64() + (index as u64) * (GDT_MAX_SIZE * size_of::<u64>()) as u64;
    unsafe { GdtBuilder::new(VirtAddr::new(new_gdt_base)) }
}
