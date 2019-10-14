mod mapper;

pub use self::mapper::PageMap;

use core::mem;
use x86_64::registers::control::{Cr0, Cr0Flags, Cr3};
use x86_64::registers::model_specific::{Efer, EferFlags};
use x86_64::structures::paging as pg;
use x86_64::structures::paging::PageTable;
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::{PhysAddr, VirtAddr};

use d7alloc;

use crate::elf_parser;
use crate::elf_parser::ELFData;
use crate::interrupt::idt;
use crate::vga_buffer::VGA_BUFFER_PHYSADDR;

use super::constants::BOOT_TMP_PAGE_TABLE_P4;
use super::prelude::*;
use super::{dma_allocator, Mapper, Page, PhysFrame};

pub unsafe fn enable_nxe() {
    Efer::update(|flags| flags.set(EferFlags::NO_EXECUTE_ENABLE, true))
}

pub unsafe fn enable_write_protection() {
    Cr0::update(|flags| {
        flags.set(Cr0Flags::WRITE_PROTECT, true);
    })
}

/// Maps elf executable based om program headers
pub fn identity_map_elf(table: &mut PageMap, elf_metadata: ELFData, flush: bool) {
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
                let mflush = unsafe { table.identity_map(frame, flags) };
                if flush {
                    mflush.flush();
                } else {
                    mflush.ignore();
                }
            }
        }
    }
}

/// Returns a mutable reference to the active level 4 table.
///
/// # Unsafety
/// The caller must guarantee that the complete physical memory is mapped to
/// virtual memory at the passed `physical_memory_offset`.
///
/// Must be only called once to avoid aliasing `&mut` references (UB)
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

/// Remap kernel and other necessary memory areas
#[must_use]
pub unsafe fn init(elf_metadata: ELFData) -> PageMap {
    rprintln!("Remapping kernel...");

    // Create new page table
    let mut new_table = unsafe { PageMap::init() };

    // Kernel code and data segments
    identity_map_elf(&mut new_table, elf_metadata, false);

    // Identity map IDT & IDTr
    let idt_frame = PhysFrame::containing_address(PhysAddr::new(idt::ADDRESS as u64));
    unsafe {
        new_table
            .identity_map(
                idt_frame,
                Flags::PRESENT | Flags::WRITABLE, // | Flags::NO_EXECUTE,
            )
            .ignore();
    }

    let idtr_frame = PhysFrame::containing_address(PhysAddr::new(idt::R_ADDRESS as u64));
    unsafe {
        new_table
            .identity_map(
                idtr_frame,
                Flags::PRESENT | Flags::WRITABLE, // | Flags::NO_EXECUTE,
            )
            .ignore();
    }

    // Identity map the VGA text buffer
    let vga_buffer_frame = PhysFrame::containing_address(VGA_BUFFER_PHYSADDR);
    unsafe {
        new_table
            .identity_map(
                vga_buffer_frame,
                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
            )
            .ignore();
    }

    // Identity map DMA memory allocator
    let start_frame = PhysFrame::containing_address(dma_allocator::BASE);
    let end_frame = PhysFrame::containing_address(dma_allocator::BASE + (dma_allocator::SIZE - 1));
    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        unsafe {
            new_table
                .identity_map(frame, Flags::WRITABLE | Flags::PRESENT | Flags::NO_EXECUTE)
                .ignore();
        }
    }

    rprintln!("Switching to new table...");
    unsafe {
        new_table.activate();
    }
    rprintln!("Remapping done.");
    new_table
}
