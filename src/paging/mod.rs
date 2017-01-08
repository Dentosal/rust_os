mod entry;
mod page;
mod table;
mod page_table;
mod mapper;
mod tlb;

use vga_buffer::VGA_BUFFER_ADDRESS;
use mem_map::{FrameAllocator, Frame, MEM_PAGE_SIZE_BYTES};
use mem_map::{MEM_PAGE_MAP_SIZE_BYTES, MEM_PAGE_MAP1_ADDRESS, MEM_PAGE_MAP2_ADDRESS};
use elf_parser;
use elf_parser::{ELFData, ELFProgramHeader};

pub use self::mapper::Mapper;
use self::page_table::{ActivePageTable,InactivePageTable};
use self::page::{Page,TemporaryPage};


const ENTRY_COUNT: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;


fn remap_kernel<A>(allocator: &mut A, elf_metadata: ELFData) where A: FrameAllocator {
    let mut temporary_page = TemporaryPage::new(Page { index: 0xcafebabe }, allocator);

    let mut active_table = unsafe { ActivePageTable::new() };
    let mut new_table = {
        let frame = allocator.allocate_frame().expect("no more frames");
        InactivePageTable::new(frame, &mut active_table, &mut temporary_page)
    };

    rprintln!("Remapping the kernel now...");

    active_table.with(&mut new_table, &mut temporary_page, |mapper| {
        for ph in elf_metadata.ph_table.iter().filter_map(|x| *x) {
            if ph.loadable() {
                let start = ph.virtual_address as usize;
                let size = ph.size_in_memory as usize;
                let mut flags = entry::PRESENT;

                if !(ph.flags.contains(elf_parser::EXECUTABLE)) {
                    flags |= entry::NO_EXECUTE;
                }
                if !ph.flags.contains(elf_parser::READABLE) {
                    panic!("Non-readable pages are not (yet) handled");
                }
                if ph.flags.contains(elf_parser::WRITABLE) {
                    flags |= entry::WRITABLE;
                }

                assert!(start % MEM_PAGE_SIZE_BYTES == 0, "Segments must be page aligned");

                rprintln!("{:#x} + {:#x} [{:?}]", start, size, flags);

                let start_frame = Frame::containing_address(start);
                let end_frame = Frame::containing_address(start + size - 1);
                for frame in Frame::range_inclusive(start_frame, end_frame) {
                    mapper.identity_map(frame, flags, allocator);
                }
            }
        }

        // identity map the VGA text buffer
        let vga_buffer_frame = Frame::containing_address(VGA_BUFFER_ADDRESS);
        mapper.identity_map(vga_buffer_frame, entry::WRITABLE | entry::PRESENT, allocator);

        // identity map the physical memory allocatior bitmaps
        let start_frame = Frame::containing_address(MEM_PAGE_MAP1_ADDRESS);
        let end_frame = Frame::containing_address(MEM_PAGE_MAP1_ADDRESS + MEM_PAGE_MAP_SIZE_BYTES - 1);
        for frame in Frame::range_inclusive(start_frame, end_frame) {
            mapper.identity_map(frame, entry::WRITABLE | entry::PRESENT, allocator);
        }

        let start_frame = Frame::containing_address(MEM_PAGE_MAP2_ADDRESS);
        let end_frame = Frame::containing_address(MEM_PAGE_MAP2_ADDRESS + MEM_PAGE_MAP_SIZE_BYTES - 1);
        for frame in Frame::range_inclusive(start_frame, end_frame) {
            mapper.identity_map(frame, entry::WRITABLE | entry::PRESENT, allocator);
        }
    });
    rprintln!("Switching...");
    let old_table = active_table.switch(new_table);
    rprintln!("Remapping done.");
}

unsafe fn enable_nxe() {
    let nxe_bit = 1 << 11;
    let efer = 0xC0000080;
    msr!(efer, msr!(efer) | nxe_bit);
}

unsafe fn enable_write_protection() {
    let wp_bit = 1 << 16;
    register!(cr0, register!(cr0) | wp_bit);
}

pub fn init(elf_metadata: ELFData) {
    unsafe {
        enable_nxe();
        enable_write_protection();
    }

    remap_kernel(&mut ALLOCATOR!(), elf_metadata);

    rprintln!("IT WORKED!");
}



pub fn test_paging() {
    let page_table = unsafe { Mapper::new() };

    // test it
    // address 0 is mapped
    rprintln!("Some = {:?}", page_table.translate_page(0));
     // second P1 entry
    rprintln!("Some = {:?}", page_table.translate_page(4096));
    // second P2 entry
    rprintln!("Some = {:?}", page_table.translate_page(512 * 4096));
    // 300th P2 entry
    rprintln!("Some = {:?}", page_table.translate_page(300 * 512 * 4096));
    // second P3 entry
    rprintln!("None = {:?}", page_table.translate_page(512 * 512 * 4096));
    // last mapped byte
    rprintln!("Some = {:?}", page_table.translate_page(512 * 512 * 4096 - 1));


}
