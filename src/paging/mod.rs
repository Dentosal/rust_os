pub mod entry;
pub mod page;
pub mod page_table;
mod table;
mod mapper;
mod tlb;

use vga_buffer::VGA_BUFFER_ADDRESS;
use interrupt::idt;
use mem_map::{FrameAllocator, Frame, MEM_PAGE_SIZE_BYTES};
use mem_map::{MEM_PAGE_MAP_SIZE_BYTES, MEM_PAGE_MAP1_ADDRESS, MEM_PAGE_MAP2_ADDRESS};
use elf_parser;
use elf_parser::{ELFData, ELFProgramHeader};

pub use self::mapper::Mapper;
pub use self::page_table::{ActivePageTable,InactivePageTable};
use self::page::{Page,TemporaryPage};


const ENTRY_COUNT: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;


pub fn remap_kernel<A>(allocator: &mut A, elf_metadata: ELFData) -> ActivePageTable where A: FrameAllocator {
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

                rprintln!("{:#x} :+ {:#x} [{:?}]", start, size, flags);

                let start_frame = Frame::containing_address(start);
                let end_frame = Frame::containing_address(start + size - 1);
                for frame in Frame::range_inclusive(start_frame, end_frame) {
                    mapper.identity_map(frame, flags, allocator);
                }
            }
        }

        // identity map IDT & IDTr
        let idt_frame = Frame::containing_address(idt::ADDRESS);
        mapper.identity_map(idt_frame, entry::WRITABLE | entry::PRESENT, allocator);

        let idtr_frame = Frame::containing_address(idt::R_ADDRESS);
        mapper.identity_map(idtr_frame, entry::WRITABLE | entry::PRESENT, allocator);

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

    active_table
}

pub unsafe fn enable_nxe() {
    let nxe_bit = 1 << 11;
    let efer: u64 = 0xC0000080;
    msr!(efer, msr!(efer) | nxe_bit);
}

pub unsafe fn enable_write_protection() {
    let wp_bit = 1 << 16;
    register!(cr0, register!(cr0) | wp_bit);
}
