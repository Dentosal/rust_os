use x86_64::PrivilegeLevel::{self, Ring0};
use x86_64::VirtAddr;

// These constants MUST have defined with same values as those in build/constants.asm
// They also MUST match the ones in plan.md
// If a constant defined here doesn't exists in that file, then it's also fine
const GDT_SELECTOR_CODE: u16 = 0x08;
pub const ADDRESS: usize = 0;
pub const VIRT_ADDRESS: VirtAddr = VirtAddr::new_truncate(ADDRESS as u64);
pub const ENTRY_COUNT: usize = 0x100;

/// http://wiki.osdev.org/IDT#Structure_AMD64
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Descriptor {
    pointer_low: u16,
    gdt_selector: u16,
    ist_offset: u8,
    options: u8,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
}

impl Descriptor {
    pub fn new(
        present: bool, pointer: u64, ring: PrivilegeLevel, ist_index: Option<u8>,
    ) -> Descriptor {
        let ist_offset = if let Some(i) = ist_index { i + 1 } else { 0 };
        assert!(ist_offset < 0b1000);
        assert!(present || (pointer == 0 && ring == Ring0)); // pointer and ring must be 0 if not present
        // example options: present => 1, ring 0 => 00, interrupt gate => 0, interrupt gate => 1110
        let options: u8 =
            0b0_00_0_1110 | ((ring as u8) << 5) | ((if present { 1 } else { 0 }) << 7);

        Descriptor {
            pointer_low: (pointer & 0xffff) as u16,
            gdt_selector: GDT_SELECTOR_CODE,
            ist_offset,
            options,
            pointer_middle: ((pointer & 0xffff_0000) >> 16) as u16,
            pointer_high: ((pointer & 0xffff_ffff_0000_0000) >> 32) as u32,
            reserved: 0,
        }
    }
}
