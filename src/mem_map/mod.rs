pub fn load_memory_map() -> bool {
    // load memory map from 0x1000-2, where out bootloader left it
    let entry_count: u8 = unsafe {*((0x1000-2) as *mut u8)};
    let base = 0x1000 as *mut u8;
    rprintln!("Memory sections:");
    for index in 0..(entry_count as isize) {
        let entry_start:    u64 = unsafe { *(base.offset(24*index+ 0) as *mut u64) };
        let entry_size:     u64 = unsafe { *(base.offset(24*index+ 8) as *mut u64) };
        let entry_type:     u32 = unsafe { *(base.offset(24*index+16) as *mut u32) };
        let acpi_data:      u32 = unsafe { *(base.offset(24*index+20) as *mut u32) };
        rprintln!("Section {}: {:#016x}-{:#016x}: type: {:#x}, acpi: {:#x}", index, entry_start, entry_start+entry_size, entry_type, acpi_data)
    }
    true
}
