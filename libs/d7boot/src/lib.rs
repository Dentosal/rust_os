// Code style
#![forbid(private_in_public)]
#![forbid(bare_trait_objects)]
// #![deny(unused_assignments)]
// Code style (development time)
#![allow(unused_macros)]
// Use std only in tests
#![cfg_attr(not(test), no_std)]
// Safety
#![deny(overflowing_literals)]
#![deny(unused_must_use)]
// Unstable features
#![feature(lang_items)]
#![feature(naked_functions)]

use core::arch::asm;
use core::intrinsics::copy_nonoverlapping;
use core::{panic::PanicInfo, ptr};

mod ata_pio;

macro_rules! sizeof {
    ($t:ty) => {{ ::core::mem::size_of::<$t>() }};
}

macro_rules! panic_indicator {
    ($x:expr) => ({
        asm!(
            concat!("mov eax, ", stringify!($x), "; mov [0xb809c], eax"),
            out("eax") _,
            options(nostack)
        );
    });
    () => ({
        panic_indicator!(0x4f214f70);   // !p
    });
}

// Keep in sync with plan.md and constants/1_boot.toml
const ELF_LOADPOINT: usize = 0x10_0000;
const INITRD_SPLIT_SECTOR_PTR: usize = 0x3000;
const INITRD_END_SECTOR_PTR: usize = 0x3004;
const KERNEL_ENTRY_POINT: u64 = 0x100_0000;
const PAGE_SIZE_BYTES: u64 = 0x20_0000;
const BOOTLOADER_SECTOR_COUNT: usize = 6;

/// Align upwards to page size
pub fn page_align_up(addr: u64) -> u64 {
    (addr + PAGE_SIZE_BYTES - 1) & !(PAGE_SIZE_BYTES - 1)
}

/// Write 'ER:_' to top top right of screen in red
/// _ denotes the argument
#[cfg(not(test))]
fn error(c: char) -> ! {
    unsafe {
        // 'ER: _'
        asm!(
            "mov [0xb8000 + 76*2], rax",
            in("rax") 0x4f5f4f3a4f524f45u64,
            options(nostack)
        );
        // set arg
        asm!(
            "mov [0xb8006 + 76*2], al",
            in("al") c as u8,
            options(nostack)
        );
        loop {
            asm!("hlt");
        }
    }
}

/// Not used when testing
#[cfg(test)]
fn create_progress_indicator() {}

/// Not used when testing
#[cfg(test)]
fn progress_indicator(_a: u8) {}

#[inline(always)]
unsafe fn check_elf() {
    // https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#File_header
    // Check that this is an ELF file (Magic number)
    if ptr::read(ELF_LOADPOINT as *const u32) != 0x464c457f {
        error('M');
    }

    // Check that the kernel entry point is correct (0x_100_0000, 1MiB)
    if ptr::read((ELF_LOADPOINT + 0x18) as *const u64) != KERNEL_ENTRY_POINT {
        error('P');
    }
}

#[no_mangle]
pub unsafe extern "C" fn d7boot() {
    // Location info
    let initrd_start_sector = (*(INITRD_SPLIT_SECTOR_PTR as *const u32)) as usize;
    let initrd_end_sector = (*(INITRD_END_SECTOR_PTR as *const u32)) as usize;

    // Load disk sectors
    let supported_sector_count = ata_pio::init();
    if (supported_sector_count as usize) < initrd_end_sector {
        // Not enough sectors
        error('S');
    }
    let mut dst: *mut u16 = ELF_LOADPOINT as *mut u16;
    for lba in BOOTLOADER_SECTOR_COUNT..initrd_end_sector {
        ata_pio::read_lba(lba as u64, 1, dst);
        dst = dst.add(0x100);
    }

    // Load kernel ELF image
    check_elf();

    // Go through the program header table
    // Just assume that the header has standard lengths and positions
    let program_header_count: u16 = ptr::read((ELF_LOADPOINT + 0x38) as *const u16);

    // Load and copy sectors
    let mut max_vaddr = 0;
    for i in 0..program_header_count {
        let base = ELF_LOADPOINT + 0x40 + i as usize * 0x38;

        let p_type = ptr::read(base as *const u32);
        // LOAD
        if p_type == 1 {
            // Read values
            let p_offset = ptr::read((base + 0x08) as *const u64);
            let p_vaddr = ptr::read((base + 0x10) as *const u64);
            let p_filesz = ptr::read((base + 0x20) as *const u64);
            let p_memsz = ptr::read((base + 0x28) as *const u64);

            if max_vaddr < p_vaddr {
                max_vaddr = p_vaddr + p_memsz;
            }

            // Clear p_memsz bytes at p_vaddr to 0
            ptr::write_bytes(p_vaddr as *mut u8, 0, p_memsz as usize);

            // Copy p_filesz bytes from p_offset to p_vaddr
            copy_nonoverlapping(
                (ELF_LOADPOINT as u64 + p_offset) as *const u8,
                p_vaddr as *mut u8,
                p_filesz as usize,
            );
        }
    }

    // Copy InitRD to be just after the kernel
    let src_ptr =
        (ELF_LOADPOINT + (initrd_start_sector - BOOTLOADER_SECTOR_COUNT) * 0x200) as *const u8;
    let dst_ptr = page_align_up(max_vaddr) as *mut u8;
    let count = (initrd_end_sector - initrd_start_sector) * 0x200;
    copy_nonoverlapping(src_ptr, dst_ptr, count);

    // Show message ('-> K') and jump to kernel
    asm!("mov [0xb8000], rax", in("rax") 0x0f4b0f200f3e0f2du64, options(nostack));
    // Set rax = 0 to signal that this is the BSP i.e. cpu0
    asm!(
        concat!("push ", 0x100_0000, "; ret"),
        in("rcx") 0
    ); // KERNEL_ENTRY_POINT
    core::hint::unreachable_unchecked();
}

#[cfg(not(test))]
#[panic_handler]
#[allow(unused_variables)]
#[no_mangle]
extern "C" fn panic(info: &PanicInfo) -> ! {
    unsafe {
        panic_indicator!(0x4f214f45); // E!
        loop {
            asm!("hlt");
        }
    }
}
