use core::ptr;

use d7alloc::{HEAP_SIZE, HEAP_START};

use super::area::PhysMemoryRange;
use super::constants::*;
use super::page_align;
use super::prelude::*;

/// Maximum number of ok-to-use entries
pub const MAX_OK_ENTRIES: usize = 20;

#[rustfmt::skip]
fn read_item(index: usize) -> (u64, u64, u32, u32) {
    let base = (BOOT_TMP_MMAP_BUFFER + 2u64).as_u64() as *mut u8;
    let e_start:        u64 = unsafe { ptr::read_unaligned(base.add(24*index     ) as *mut u64) };
    let e_size:         u64 = unsafe { ptr::read_unaligned(base.add(24*index +  8) as *mut u64) };
    let e_type:         u32 = unsafe { ptr::read_unaligned(base.add(24*index + 16) as *mut u32) };
    let e_acpi_data:    u32 = unsafe { ptr::read_unaligned(base.add(24*index + 20) as *mut u32) };
    (e_start, e_size, e_type, e_acpi_data)
}

pub(crate) fn load_memory_map() -> [Option<PhysMemoryRange>; MAX_OK_ENTRIES] {
    // load memory map from where out bootloader left it
    // http://wiki.osdev.org/Detecting_Memory_(x86)#BIOS_Function:_INT_0x15.2C_EAX_.3D_0xE820

    let mut ok_entries: [Option<PhysMemoryRange>; MAX_OK_ENTRIES] = [None; MAX_OK_ENTRIES];

    let mut out_entry_index: usize = 0;

    {
        let mut write_entry = |entry| {
            for i in 0..MAX_OK_ENTRIES {
                if let Some(ok) = ok_entries[i] {
                    if ok.can_merge(entry) {
                        ok_entries[i] = Some(ok.merge(entry));
                        return;
                    }
                }
            }

            assert!(out_entry_index < MAX_OK_ENTRIES);
            ok_entries[out_entry_index] = Some(entry);
            out_entry_index += 1;
        };

        let mut split_and_write_entry = |entry: PhysMemoryRange| {
            // These are permanently reserved for the kernel
            if let Some(ok) = entry.above(MEMORY_RESERVED_BELOW) {
                // These are permanently reserved for the heap
                if let Some(below) = ok.below(PhysAddr::new(HEAP_START)) {
                    write_entry(below);
                }
                if let Some(above) = ok.above(PhysAddr::new(HEAP_START + HEAP_SIZE)) {
                    write_entry(above);
                }
            }
        };

        let entry_count: u8 =
            unsafe { ptr::read_volatile(BOOT_TMP_MMAP_BUFFER.as_u64() as *mut u8) };
        for index in 0..(entry_count as usize) {
            let (e_start, e_size, e_type, e_acpi_data) = read_item(index);
            log::trace!(
                "Section {:>3}: {:>16x}-{:>16x}: type: {:#x}, acpi: {:#x}",
                index,
                e_start,
                e_start + e_size,
                e_type,
                e_acpi_data
            );

            // Types 1, 4 ok to use and acpi_data bit 0 must be set
            let usable = (e_type == 1 || e_type == 4) && (e_acpi_data & 1) == 1;

            if !usable {
                continue;
            }

            // set frame data. accept only full frames
            let start = page_align(PhysAddr::new(e_start), true);
            let end = page_align(PhysAddr::new(e_start + e_size), false);

            if start == end {
                continue;
            }

            split_and_write_entry(PhysMemoryRange::range(start..end));
        }
    }

    // TODO: Check that required memory regions exist

    // Calculate and display memory size
    let mut memory_counter_bytes: u64 = 0;
    for entry in ok_entries.iter() {
        if let Some(area) = entry {
            memory_counter_bytes += area.size_bytes() as u64;
            log::debug!("Area       : {:>16x}-{:>16x}", area.start(), area.end());
        }
    }

    if memory_counter_bytes < 1024 * 1024 * 1024 {
        log::info!("Memory size {} MiB", memory_counter_bytes / (1024 * 1024));
    } else {
        let full_gibs = memory_counter_bytes / (1024 * 1024 * 1024);
        let cent_gibs = (memory_counter_bytes % (1024 * 1024 * 1024)) / 1024_00_000;
        log::info!("Memory size {}.{:02} GiB", full_gibs, cent_gibs);
    }

    // Result
    ok_entries
}
