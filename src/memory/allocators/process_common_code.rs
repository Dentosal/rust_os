use core::intrinsics::copy_nonoverlapping;
use core::ptr;
use x86_64::structures::paging::PageTableFlags as Flags;

use crate::memory::{self, phys_to_virt, prelude::*};

use super::super::MemoryController;

pub static mut COMMON_ADDRESS_PHYS: u64 = 0; // Temp value
pub const COMMON_ADDRESS_VIRT: u64 = 0x20_0000;

pub static mut PROCESS_IDT_PHYS_ADDR: u64 = 0; // Temp value

unsafe fn load_common_code(mem_ctrl: &mut MemoryController) {
    let common_addr = VirtAddr::new_unsafe(COMMON_ADDRESS_VIRT);

    let bytes = crate::initrd::read("p_commoncode").expect("p_commoncode missing from initrd");
    assert!(bytes.len() <= (PAGE_SIZE_BYTES as usize));

    let frame = mem_ctrl
        .frame_allocator
        .allocate_frame()
        .expect("Could not allocate frame");

    mem_ctrl
        .page_map
        .map_to(
            PT_VADDR,
            Page::from_start_address(common_addr).unwrap(),
            frame,
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        )
        .flush();

    let base: *mut u8 = common_addr.as_mut_ptr();
    copy_nonoverlapping(bytes.as_ptr(), base, bytes.len());

    mem_ctrl
        .page_map
        .map_to(
            PT_VADDR,
            Page::from_start_address(common_addr).unwrap(),
            frame,
            Flags::PRESENT,
        )
        .flush();

    COMMON_ADDRESS_PHYS = frame.start_address().as_u64();
}

/// Create process descriptor tables
/// These are shared for all processes
unsafe fn create_process_dts(mem_ctrl: &mut MemoryController) {
    use crate::interrupt::write_process_dts;

    // Find process_interrupt.table_start
    let p = COMMON_ADDRESS_VIRT as *const u64;
    let interrupt_table_start = VirtAddr::new_unsafe(ptr::read(p.offset(1)));

    // Allocate memory
    let frame = mem_ctrl
        .frame_allocator
        .allocate_frame()
        .expect("Could not allocate frame");

    let paddr = frame.start_address();

    PROCESS_IDT_PHYS_ADDR = paddr.as_u64();

    write_process_dts(phys_to_virt(paddr), interrupt_table_start);
}

/// Must be called when disk driver (and staticfs) are available
pub fn init() {
    memory::configure(|mem_ctrl: &mut MemoryController| unsafe {
        load_common_code(mem_ctrl);
        create_process_dts(mem_ctrl);
    });
}
