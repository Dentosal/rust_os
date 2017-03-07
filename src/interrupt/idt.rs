use core::ptr;
use core::mem;

// These constants MUST have defined with same values as those in src/asm_routines/constants.asm
// They also MUST match the ones in plan.md
// If a constant defined here doesn't exists in that file, then it's also fine
const GDT_SELECTOR_CODE: u16 = 0x08;
pub const IDT_ADDRESS: usize = 0x0;
pub const IDTR_ADDRESS: usize = 0x1000;
pub const IDT_ENTRY_COUNT: usize = 0x100;


#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IDTReference {
    limit: u16,
    offset: u64
}
impl IDTReference {
    pub fn new() -> IDTReference {
        IDTReference {
            limit: ((IDT_ENTRY_COUNT-1)*(mem::size_of::<IDTDescriptor>())) as u16,
            offset: IDT_ADDRESS as u64
        }
    }
    pub unsafe fn write(&self) {
        ptr::write_volatile(IDTR_ADDRESS as *mut Self, *self);
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IDTDescriptor {
    pointer_low: u16,
    gdt_selector: u16,
    zero: u8,
    options: u8,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32
}

impl IDTDescriptor {
    pub fn new(present: bool, pointer: u64, ring: u8) -> IDTDescriptor {
        assert!(ring < 4);
        assert!(present || (pointer == 0 && ring == 0)); // pointer and ring must be 0 if not present
        // example options: present => 1, ring 0 => 00, interrupt gate => 0, interrupt gate => 1110,
        let options: u8 = 0b0_00_0_1110 | (ring << 5) | ((if present {1} else {0}) << 7);

        IDTDescriptor {
            pointer_low: (pointer & 0xffff) as u16,
            gdt_selector: GDT_SELECTOR_CODE,
            zero: 0,
            options: options,
            pointer_middle: ((pointer & 0xffff_0000) >> 16) as u16,
            pointer_high: ((pointer & 0xffff_ffff_0000_0000) >> 32) as u32,
            reserved: 0,
        }
    }
}
