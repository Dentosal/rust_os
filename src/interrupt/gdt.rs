// See http://os.phil-opp.com/double-faults.html#switching-stacks for more info

use core::mem::size_of;
use core::ptr;

use x86_64::structures::gdt::SegmentSelector;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::{PrivilegeLevel, VirtAddr};

const MAX_ENTRIES: usize = 8;
pub const DOUBLE_FAULT_IST_INDEX: usize = 0;

bitflags! {
    struct DescriptorFlags: u64 {
        const CONFORMING        = 1 << 42;
        const EXECUTABLE        = 1 << 43;
        const USER_SEGMENT      = 1 << 44;
        const PRESENT           = 1 << 47;
        const LONG_MODE         = 1 << 53;
    }
}

pub enum Descriptor {
    UserSegment(u64),
    SystemSegment(u64, u64),
}

impl Descriptor {
    pub fn kernel_code_segment() -> Descriptor {
        let flags = DescriptorFlags::USER_SEGMENT
            | DescriptorFlags::PRESENT
            | DescriptorFlags::EXECUTABLE
            | DescriptorFlags::LONG_MODE;
        Descriptor::UserSegment(flags.bits())
    }
    pub fn tss_segment(tss: &'static TaskStateSegment) -> Descriptor {
        use bit_field::BitField;

        let ptr = tss as *const _ as u64;

        let mut low = DescriptorFlags::PRESENT.bits();
        // base
        low.set_bits(16..40, ptr.get_bits(0..24));
        low.set_bits(56..64, ptr.get_bits(24..32));
        low.set_bits(0..16, (size_of::<TaskStateSegment>() - 1) as u64);
        // type (0b1001 = available 64-bit tss)
        low.set_bits(40..44, 0b1001);

        let mut high = 0;
        high.set_bits(0..32, ptr.get_bits(32..64));

        Descriptor::SystemSegment(low, high)
    }
}

pub struct GdtBuilder {
    addr: VirtAddr,
    entry_count: usize,
}
impl GdtBuilder {
    pub unsafe fn new(addr: VirtAddr) -> GdtBuilder {
        GdtBuilder {
            addr,
            entry_count: 1, // first entry is the zero descriptor, so it is not free
        }
    }

    pub fn add_entry(&mut self, entry: Descriptor) -> SegmentSelector {
        assert!(self.entry_count < MAX_ENTRIES, "GDT Full");

        let base: *mut u64 = self.addr.as_mut_ptr();
        let index = self.entry_count;
        unsafe {
            match entry {
                Descriptor::UserSegment(value) => {
                    ptr::write(base.add(self.entry_count), value);
                    self.entry_count += 1;
                },
                Descriptor::SystemSegment(value_low, value_high) => {
                    ptr::write(base.add(self.entry_count), value_low);
                    self.entry_count += 1;
                    ptr::write(base.add(self.entry_count), value_high);
                    self.entry_count += 1;
                },
            };
        }
        SegmentSelector::new(index as u16, PrivilegeLevel::Ring0)
    }

    pub unsafe fn load(&'static self) {
        use core::mem::size_of;
        use x86_64::instructions::tables::{lgdt, DescriptorTablePointer};

        let ptr = DescriptorTablePointer {
            base: self.addr.as_ptr() as *const u64 as u64,
            limit: (self.entry_count * size_of::<u64>() - 1) as u16,
        };

        lgdt(&ptr);
    }
}
