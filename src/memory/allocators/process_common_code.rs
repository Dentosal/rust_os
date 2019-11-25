use core::ptr;
use x86_64::structures::paging::PageTableFlags as Flags;

use crate::filesystem::FILESYSTEM;
use crate::memory::{self, prelude::*};

use super::super::MemoryController;

pub static mut COMMON_ADDRESS_PHYS: u64 = 0; // Temp value
pub const COMMON_ADDRESS_VIRT: u64 = 0x20_0000;

pub static mut PROCESS_IDT_PHYS_ADDR: u64 = 0;

unsafe fn load_common_code(mem_ctrl: &mut MemoryController) {
    let common_addr = VirtAddr::new_unchecked(COMMON_ADDRESS_VIRT);

    let bytes = FILESYSTEM
        .lock()
        .read_file("/mnt/staticfs/p_commoncode")
        .expect("p_commoncode: file unavailable");
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
    for (offset, byte) in bytes.into_iter().enumerate() {
        ptr::write(base.add(offset), byte);
    }

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
unsafe fn create_process_dts(mem_ctrl: &mut MemoryController) {
    use crate::interrupt::write_process_dts;

    // Find process_interrupt.table_start
    let p = COMMON_ADDRESS_VIRT as *const u64;
    let interrupt_table_start = VirtAddr::new_unchecked(ptr::read(p.offset(1)));

    // Allocate memory
    let (frames, area) =
        mem_ctrl.alloc_both(1, Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE);
    let frame = frames[0];

    PROCESS_IDT_PHYS_ADDR = frame.start_address().as_u64();

    write_process_dts(area.start, interrupt_table_start);

    // Remap to remove write flag (TODO: unmap?)
    mem_ctrl
        .page_map
        .map_to(
            PT_VADDR,
            Page::from_start_address(area.start).unwrap(),
            frame,
            Flags::PRESENT | Flags::NO_EXECUTE,
        )
        .flush();
}

/// Must be called when disk driver (and staticfs) are available
pub fn init() {
    memory::configure(|mem_ctrl: &mut MemoryController| unsafe {
        load_common_code(mem_ctrl);
        create_process_dts(mem_ctrl);
    });
}
