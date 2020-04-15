// Code style
#![forbid(private_in_public)]
#![forbid(bare_trait_objects)]
// #![deny(unused_assignments)]
// Code style (development time)
#![allow(unused_macros)]
#![allow(dead_code)]
// Code style (temp)
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(unused_mut)]
#![allow(unused_unsafe)]
#![allow(unreachable_code)]
// Use std only in tests
#![cfg_attr(not(test), no_std)]
// Safety
#![deny(overflowing_literals)]
#![deny(safe_packed_borrows)]
#![deny(unused_must_use)]
// Unstable features
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]

#[cfg(not(test))]
use core::{mem, panic::PanicInfo, ptr};
#[cfg(test)]
use std::{mem, panic::PanicInfo, ptr};


macro_rules! sizeof {
    ($t:ty) => {{ ::core::mem::size_of::<$t>() }};
}

macro_rules! panic_indicator {
    ($x:expr) => ({
        asm!(concat!("mov eax, ", stringify!($x), "; mov [0xb809c], eax") ::: "eax", "memory" : "volatile", "intel");
    });
    () => ({
        panic_indicator!(0x4f214f70);   // !p
    });
}

// Keep in sync with src/asm_routines/constants.asm
const ELF_LOADPOINT: usize = 0x10_000;

/// Write 'ER:_' to top top right of screen in red
/// _ denotes the argument
#[cfg(not(test))]
fn error(c: char) -> ! {
    unsafe {
        // 'ER: _'
        asm!("mov rax, 0x4f5f4f3a4f524f45; mov [0xb8000 + 76*2], rax" ::: "rax", "memory" : "volatile", "intel");
        // set arg
        asm!("mov [0xb8006 + 76*2], al" :: "{al}"(c as u8) : "al", "memory" : "volatile", "intel");
        asm!("hlt" :::: "volatile", "intel");
        core::hint::unreachable_unchecked();
    }
}

/// Test error
#[cfg(test)]
fn error(a: char) -> ! {
    panic!("ERR: {}", a);
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
    if ptr::read((ELF_LOADPOINT + 0x18) as *const u64) != 0x100_0000 {
        error('P');
    }
}

#[naked]
#[no_mangle]
pub unsafe extern "C" fn d7boot(a: u8) {
    check_elf();

    // Go through the program header table
    // Just assume that the header has standard lengths and positions
    let program_header_count: u16 = ptr::read((ELF_LOADPOINT + 0x38) as *const u16);

    // Load and decompress sectors
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

            // Clear p_memsz bytes at p_vaddr to 0
            ptr::write_bytes(p_vaddr as *mut u8, 0, p_memsz as usize);

            // Copy p_filesz bytes from p_offset to p_vaddr
            core::intrinsics::copy_nonoverlapping(
                (ELF_LOADPOINT as u64 + p_offset) as *const u8,
                p_vaddr as *mut u8,
                p_filesz as usize,
            );
        }
    }

    // Show message and jump to kernel
    asm!("mov rax, 0x4f2d4f3e4f204f4b; mov [0xb8000], rax" ::: "rax", "memory" : "volatile", "intel");
    asm!(concat!("push ", 0x100_0000, "; ret") :::: "volatile", "intel");
    core::hint::unreachable_unchecked();
}

#[cfg(not(test))]
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern "C" fn eh_personality() -> ! {
    loop {}
}

#[cfg(not(test))]
#[panic_handler]
#[allow(unused_variables)]
#[no_mangle]
extern "C" fn panic(info: &PanicInfo) -> ! {
    unsafe {
        panic_indicator!(0x4f214f45); // E!
        asm!("hlt"::::"intel","volatile");
        core::hint::unreachable_unchecked();
    }
}

