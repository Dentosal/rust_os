mod mapper;

pub use self::mapper::PageMap;

use x86_64::registers::control::{Cr0, Cr0Flags};
use x86_64::registers::model_specific::{Efer, EferFlags};
use x86_64::structures::paging as pg;
use x86_64::structures::paging::PageTableFlags as Flags;
use x86_64::PhysAddr;

use crate::driver::vga_buffer::VGA_BUFFER_PHYSADDR;
use crate::interrupt::idt;
use crate::util::elf_parser::ELFData;

use super::prelude::*;
use super::{Page, PhysFrame};

pub unsafe fn enable_nxe() {
    Efer::update(|flags| flags.set(EferFlags::NO_EXECUTE_ENABLE, true))
}

pub unsafe fn enable_write_protection() {
    Cr0::update(|flags| {
        flags.set(Cr0Flags::WRITE_PROTECT, true);
    })
}

pub unsafe fn set_active_table(p4_addr: PhysAddr) {
    use x86_64::registers::control::{Cr3, Cr3Flags};
    Cr3::write(
        pg::PhysFrame::<pg::Size4KiB>::from_start_address(p4_addr).expect("Misaligned P4"),
        Cr3Flags::empty(),
    );
}

/// Remap kernel and other necessary memory areas
#[must_use]
pub unsafe fn init(elf_metadata: ELFData) -> PageMap {
    log::debug!("Remapping kernel...");

    // Create new page table
    let mut new_table = unsafe { PageMap::init(PT_VADDR, PT_PADDR, PT_VADDR) };

    // Kernel code and data segments
    new_table.identity_map_elf(PT_VADDR, elf_metadata);

    // Identity map IDT, GDT, DMA buffers, and the VGA text buffer
    let idt_frame = PhysFrame::containing_address(PhysAddr::new(idt::ADDRESS as u64));
    let vga_buffer_frame = PhysFrame::containing_address(VGA_BUFFER_PHYSADDR);

    assert_eq!(idt_frame, vga_buffer_frame);
    let lowmem_frame = idt_frame;

    unsafe {
        new_table
            .identity_map(
                PT_VADDR,
                lowmem_frame,
                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
            )
            .ignore();
    }

    log::debug!("Switching to new table...");
    unsafe {
        new_table.activate();
    }
    log::debug!("Remapping done.");
    new_table
}
