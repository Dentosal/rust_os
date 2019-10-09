use core::mem;
use x86_64::structures::paging as pg;
use x86_64::structures::paging::PageTable;
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{PhysAddr, VirtAddr};
use x86_64::registers::control::Cr3;


use d7alloc;

use crate::elf_parser;
use crate::elf_parser::ELFData;
use crate::interrupt::idt;
use crate::vga_buffer::VGA_BUFFER_PHYSADDR;

use super::constants::BOOT_TMP_PAGE_TABLE_P4;
use super::prelude::*;
use super::{dma_allocator, FrameAllocator, Mapper, Page, PhysFrame};

mod create_table;
use self::create_table::create_table;

pub unsafe fn enable_nxe() {
    let nxe_bit = 1 << 11;
    let efer: u64 = 0xC0000080;
    msr!(efer, msr!(efer) | nxe_bit);
}

pub unsafe fn enable_write_protection() {
    let wp_bit = 1 << 16;
    register!(cr0, register!(cr0) | wp_bit);
}

/// Returns the physical address for a virtual address, if mapped
pub fn translate_addr(addr: u64, table: &pg::RecursivePageTable) -> Option<PhysAddr> {
    use x86_64::structures::paging::mapper::TranslateError;

    let addr = VirtAddr::new(addr);
    let page: Page = Page::containing_address(addr);

    let pg_off: u64 = addr.page_offset().into();

    // perform the translation
    let frame = table.translate_page(page);
    match frame.map(|frame| frame.start_address() + pg_off) {
        Ok(v) => Some(v),
        Err(TranslateError::PageNotMapped) => None,
        Err(err) => {
            panic!("Page translation failed: {:?}", err);
        }
    }
}

/// Maps elf executable based om program headers
pub fn identity_map_elf<A>(
    allocator: &mut A,
    table: &mut pg::RecursivePageTable,
    elf_metadata: ELFData,
    flush: bool,
) where
    A: FrameAllocator<PageSizeType>,
{
    for ph in elf_metadata.ph_table.iter().filter_map(|x| *x) {
        if ph.loadable() {
            let start = PhysAddr::new(ph.virtual_address);
            let size = ph.size_in_memory;
            let mut flags = Flags::PRESENT;

            assert!(start.as_u64() % Page::SIZE == 0);
            assert!(size > 0);

            if !ph.has_flag(elf_parser::ELFPermissionFlags::EXECUTABLE) {
                flags |= Flags::NO_EXECUTE;
            }
            if !ph.has_flag(elf_parser::ELFPermissionFlags::READABLE) {
                panic!("Non-readable pages are not (yet) handled");
            }
            if ph.has_flag(elf_parser::ELFPermissionFlags::WRITABLE) {
                flags |= Flags::WRITABLE;
            }

            rprintln!("{:#x} :+ {:#x} [{:?}]", start, size, flags);

            let start_frame = PhysFrame::containing_address(start);
            let end_frame = PhysFrame::containing_address(start + (size - 1));
            for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
                let mflush = unsafe {
                    table
                        .identity_map(frame, flags, allocator)
                        .expect("Mapping failed")
                };
                if flush {
                    mflush.flush();
                } else {
                    mflush.ignore();
                }
            }
        }
    }
}

/// Prepare old table
// pub fn prepare<A>(allocator: &mut A, old_table: pg::RecursivePageTable)
// where
//     A: FrameAllocator<PageSizeType>,
// {
//     old_table
//         .identity_map(idt_frame, Flags::WRITABLE | Flags::PRESENT, allocator)
//         .expect("Mapping failed")
//         .flush();

// }

/// Returns a mutable reference to the active level 4 table.
///
/// # Unsafety
/// The caller must guarantee that the complete physical memory is mapped to
/// virtual memory at the passed `physical_memory_offset`.
///
/// Must be only called once to avoid aliasing `&mut` references (UB)
pub(super) unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

pub(super) unsafe fn get_table(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Remap kernel and other necessary memory area
/// Returns address of the new page table
pub fn init<A>(allocator: &mut A, elf_metadata: ELFData) -> PhysAddr
where
    A: FrameAllocator<PageSizeType>,
{
    // Create new page table
    let p4_address = create_table(allocator);
    let mut new_p4_table = unsafe { &mut *(p4_address.as_u64() as *mut PageTable) };

    rprintln!("Switching to new table...");
    unsafe {
        register!(cr3, p4_address.as_u64());
    }

    rprintln!("Remapping kernel...");

    let mut new_table = pg::RecursivePageTable::new(&mut new_p4_table).expect("Invalid page table");

    // Kernel code and data segments
    identity_map_elf(allocator, &mut new_table, elf_metadata, false);

    // Identity map IDT & IDTr
    let idt_frame = PhysFrame::containing_address(PhysAddr::new(idt::ADDRESS as u64));
    unsafe {
        new_table
            .identity_map(idt_frame, Flags::WRITABLE | Flags::PRESENT, allocator)
            .expect("Mapping failed")
            .ignore();
    }

    let idtr_frame = PhysFrame::containing_address(PhysAddr::new(idt::R_ADDRESS as u64));
    unsafe {
        new_table
            .identity_map(idtr_frame, Flags::WRITABLE | Flags::PRESENT, allocator)
            .expect("Mapping failed")
            .ignore();
    }
    // Identity map the VGA text buffer
    let vga_buffer_frame = PhysFrame::containing_address(VGA_BUFFER_PHYSADDR);
    unsafe {
        new_table
            .identity_map(
                vga_buffer_frame,
                Flags::WRITABLE | Flags::PRESENT,
                allocator,
            )
            .expect("Mapping failed")
            .ignore();
    }

    // Identity map DMA memory allocator
    let start_frame = PhysFrame::containing_address(dma_allocator::BASE);
    let end_frame = PhysFrame::containing_address(dma_allocator::BASE + (dma_allocator::SIZE - 1));
    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        unsafe {
            new_table
                .identity_map(frame, Flags::WRITABLE | Flags::PRESENT, allocator)
                .expect("Mapping failed")
                .ignore();
        }
    }

    rprintln!("Remapping done.");

    p4_address
}
