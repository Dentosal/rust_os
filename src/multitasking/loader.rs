use crate::memory::prelude::*;
use crate::memory::Area;
use crate::util::elf_parser::*;

/// Contains a "pointer" to loaded elf image
/// Validity of the elf image must be verified when creating this structure
#[derive(Debug, Clone, Copy)]
pub struct ElfImage {
    /// Virtual memory area where the elf image is loaded
    area: Area,
}
impl ElfImage {
    pub unsafe fn new(area: Area) -> Self {
        Self { area }
    }

    pub fn parse_elf(&self) -> ELFData {
        unsafe {
            parse_elf(self.as_ptr() as usize)
                .expect("ELF image was modified to invalid state after creation")
        }
    }

    pub fn verify(&self) {
        unsafe {
            parse_elf(self.as_ptr() as usize).expect("Invalid ELF image");
        }
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.area.start.as_ptr() as *const u8
    }
}
