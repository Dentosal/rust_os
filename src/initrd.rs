//! Initial ramdisk driver

use alloc::string::String;
use alloc::vec::Vec;
use hashbrown::HashMap;
use x86_64::{PhysAddr, VirtAddr};

use d7initrd::{FileEntry, HEADER_MAGIC, HEADER_SIZE_BYTES};

use crate::memory::{self, prelude::*};
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

        // Map header, assumes that it fits in one page. Ought to be enough.
        let header_area = memory::configure(|mm| {
            let header_area = mm.alloc_virtual_area(1);
            let frame = PhysFrame::containing_address(start_addr);
            mm.map_area(header_area, &[frame], false);
            header_area
        });

        let hptr: *const u8 = header_area.start.as_ptr();
        let magic = *(hptr.add(0) as *const u32);
        let size_flist = *(hptr.add(4) as *const u32);
        let size_total = *(hptr.add(8) as *const u64);
        let size_pages = to_pages_round_up(size_total);

        assert_eq!(magic, HEADER_MAGIC, "InitRD magic mismatch");
        assert!(size_flist as u64 > 0, "InitRD header empty");
        assert!(
            size_flist as u64 + 8 < PAGE_SIZE_BYTES,
            "InitRD header too large"
        );

        let header_bytes: &[u8] = core::slice::from_raw_parts(hptr.add(16), size_flist as usize);

        let file_list: Vec<FileEntry> = pinecone::from_bytes(&header_bytes[..])
            .expect("Could not deserialize staticfs file list");

        log::trace!("Files {:?}", file_list);

        // Unmap header, and then map header + all files
        let area = memory::configure(|mm| {
            mm.unmap_area(header_area);
            mm.free_virtual_area(header_area);

            let area = mm.alloc_virtual_area(size_pages);
            let frames: Vec<_> = (0..size_pages)
                .map(|i| PhysFrame::from_start_address(start_addr + PAGE_SIZE_BYTES * i).unwrap())
                .collect();
            mm.map_area(area, &frames, false);
            area
        });

        // Initialize
        let files_offset = HEADER_SIZE_BYTES + (size_flist as usize);
        let files_len = size_total as usize - files_offset;
        let p: *const u8 = area.start.as_ptr();
        INITRD.call_once(move || InitRD {
            files: file_list.into_iter().map(|f| (f.name.clone(), f)).collect(),
            slice: core::slice::from_raw_parts(p.add(files_offset), files_len),
        });
    }
}

pub fn read(name: &str) -> Option<&'static [u8]> {
    let rd: &InitRD = INITRD.r#try().unwrap();
    log::trace!("Read {:?} (found={})", name, rd.files.contains_key(name));
    let entry = rd.files.get(name)?;
    let start = entry.offset as usize;
    let len = entry.size as usize;
    Some(&rd.slice[start..start + len])
}
