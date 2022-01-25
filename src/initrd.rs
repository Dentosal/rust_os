//! Initial ramdisk driver

use alloc::string::String;
use alloc::vec::Vec;
use hashbrown::HashMap;
use x86_64::{PhysAddr, VirtAddr};

use d7initrd::{FileEntry, HEADER_MAGIC, HEADER_SIZE_BYTES};

use crate::memory::{self, phys_to_virt, prelude::*};
use crate::util::elf_parser::{self, ELFData, ELFHeader, ELFProgramHeader};

#[derive(Debug)]
struct InitRD {
    /// Files by name.
    files: HashMap<String, FileEntry>,
    /// A slice containing all files, concatenated.
    /// The lifetime is static, as these are never deallocated.
    slice: &'static [u8],
}

static INITRD: spin::Once<InitRD> = spin::Once::new();

pub fn init(elf_data: ELFData) {
    unsafe {
        // Get address
        let start_addr = PhysAddr::from_u64(page_align_up(elf_data.last_addr()));
        let header = phys_to_virt(start_addr);

        let hptr: *const u8 = header.as_ptr();
        let magic = *(hptr.add(0) as *const u32);
        let size_flist = *(hptr.add(4) as *const u32);
        let size_total = *(hptr.add(8) as *const u64);

        assert_eq!(magic, HEADER_MAGIC, "InitRD magic mismatch");
        assert!(size_flist as u64 > 0, "InitRD header empty");
        assert!(
            size_flist as u64 + 8 < PAGE_SIZE_BYTES,
            "InitRD header too large"
        );

        let header_bytes: &[u8] = core::slice::from_raw_parts(hptr.add(16), size_flist as usize);

        let file_list: Result<Vec<FileEntry>, _> = pinecone::from_bytes(&header_bytes[..]);
        let file_list = file_list.expect("Could not deserialize staticfs file list");
        log::trace!("Files {:?}", file_list);

        // Initialize
        let files_offset = HEADER_SIZE_BYTES + (size_flist as usize);
        let files_len = size_total as usize - files_offset;
        let p: *const u8 = header.as_ptr();
        INITRD.call_once(move || InitRD {
            files: file_list.into_iter().map(|f| (f.name.clone(), f)).collect(),
            slice: core::slice::from_raw_parts(p.add(files_offset), files_len),
        });
    }
}

pub fn read(name: &str) -> Option<&'static [u8]> {
    let rd: &InitRD = INITRD.poll().unwrap();
    log::trace!("Read {:?} (found={})", name, rd.files.contains_key(name));
    let entry = rd.files.get(name)?;
    let start = entry.offset as usize;
    let len = entry.size as usize;
    Some(&rd.slice[start..start + len])
}
