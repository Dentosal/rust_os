use core::ptr;
use x86_64::structures::paging::{FrameAllocator, PageTableFlags as Flags};

use crate::elf_parser::*;
use crate::memory::prelude::*;
use crate::memory::Area;
use crate::memory::{self, MemoryController, Page, PhysFrame};
use crate::staticfs;

use alloc::prelude::v1::Vec;

/// Contains a "pointer" to loaded elf image
/// Validity of the elf image must be verified when creating this structure
#[derive(Debug, Clone, Copy)]
pub struct ElfImage {
    /// Virtual memory area where the elf image is loaded
    area: Area,
}
impl ElfImage {
    pub fn parse_elf(&self) -> ELFData {
        unsafe {
            parse_elf(self.as_ptr() as usize)
                .expect("ELF image was modified to invlaid state after creation")
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

/// Loads elf image from staticfs to ram and returns
pub fn load_module(path: &str) -> Option<ElfImage> {
    let bytes = staticfs::read_file(path)?;

    let size_pages =
        memory::page_align(PhysAddr::new(bytes.len() as u64), true).as_u64() / Page::SIZE;

    // Allocate load buffer
    let area = memory::configure(|mem_ctrl| {
        mem_ctrl.alloc_pages(size_pages as usize, Flags::PRESENT | Flags::WRITABLE)
    });

    // Store the file to buffer
    let base: *mut u8 = area.start.as_mut_ptr();
    let mut it = bytes.into_iter();
    for page_offset in 0..size_pages {
        for byte_offset in 0..PAGE_SIZE_BYTES {
            let i = page_offset * PAGE_SIZE_BYTES + byte_offset;
            unsafe {
                ptr::write(base.offset(i as isize), it.next().unwrap_or(0));
            }
        }
    }

    let elf = ElfImage { area };
    elf.verify();
    Some(elf)
}
