use core::intrinsics::copy_nonoverlapping;
use core::ptr;
use x86_64::structures::paging::PageTableFlags as Flags;

use crate::memory::{self, paging::PAGE_MAP, phys, phys_to_virt, prelude::*};

pub static mut COMMON_ADDRESS_PHYS: u64 = 0; // Temp value
pub const COMMON_ADDRESS_VIRT: u64 = 0x20_0000;

pub static mut PROCESS_IDT_PHYS_ADDR: u64 = 0; // Temp value

unsafe fn load_common_code() {
    let common_addr = VirtAddr::new_unsafe(COMMON_ADDRESS_VIRT);

    let bytes = crate::initrd::read("p_commoncode").expect("p_commoncode missing from initrd");
    assert!(bytes.len() <= (PAGE_SIZE_BYTES as usize));

    let frame_backing = phys::allocate(PAGE_LAYOUT)
        .expect("Could not allocate frame")
        .leak();

    let frame = PhysFrame::from_start_address(frame_backing.start()).unwrap();

    let mut page_map = PAGE_MAP.try_lock().unwrap();
    page_map
        .map_to(
            PT_VADDR,
            Page::from_start_address(common_addr).unwrap(),
            frame,
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        )
        .flush();

    let base: *mut u8 = common_addr.as_mut_ptr();
    copy_nonoverlapping(bytes.as_ptr(), base, bytes.len());

    page_map
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
unsafe fn create_process_dts() {
    use crate::interrupt::write_process_dts;

    // Find process_interrupt.table_start
    let p = COMMON_ADDRESS_VIRT as *const u64;
    let interrupt_table_start = VirtAddr::new_unsafe(ptr::read(p.offset(1)));

    // Allocate memory
    let paddr = phys::allocate(PAGE_LAYOUT)
        .expect("Could not allocate frame")
        .leak()
        .start();

    PROCESS_IDT_PHYS_ADDR = paddr.as_u64();

    write_process_dts(phys_to_virt(paddr), interrupt_table_start);
}

/// Must be called when disk driver (and staticfs) are available
pub unsafe fn init() {
    load_common_code();
    create_process_dts();
}
