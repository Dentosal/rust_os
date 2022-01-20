use alloc::vec::Vec;
use core::ptr;

use crate::memory::phys::OutOfMemory;
use crate::memory::{self, phys, prelude::*, Page};
use crate::util::elf_parser::*;

/// A loaded and validated elf image
#[derive(Debug)]
pub struct ElfImage {
    pub(super) header: ELFHeader,
    pub(super) sections: Vec<(ELFProgramHeader, Vec<phys::Allocation>)>,
}

/// Loads a program from ELF ímage to physical memory.
/// This function does not load the ELF to its p_vaddr, but
/// rather returns a list of unmapped physical frames.
///
/// This function internally uses TLB flushes.
///
/// Requires that the kernel page tables are active.
pub fn load_elf(image: &[u8]) -> Result<ElfImage, OutOfMemory> {
    let elf = unsafe {
        parse_elf(image).expect("Invalid ELF image") // TODO: return error
    };

    let mut frames = Vec::new();
    for ph in elf.ph_table.iter().filter_map(|x| *x) {
        if ph.loadable() && ph.size_in_memory != 0 {
            let size_in_pages = page_align_u64(ph.size_in_memory, true) / PAGE_SIZE_BYTES;
            let mut section_frames = Vec::new();
            for _ in 0..size_in_pages {
                let mut allocation = phys::allocate_zeroed(PAGE_LAYOUT)?;
                let area = allocation.write();

                // Copy p_filesz bytes from p_offset to target
                let start = ph.offset as usize;
                let size = ph.size_in_file as usize;
                area[..size].copy_from_slice(&image[start..start + size]);

                section_frames.push(allocation);
            }

            // Append frames to the result
            frames.push((ph, section_frames));
        }
    }

    Ok(ElfImage {
        header: elf.header,
        sections: frames,
    })
}
