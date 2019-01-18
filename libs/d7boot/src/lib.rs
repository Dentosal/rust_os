// Code style
#![forbid(private_in_public)]
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
#![feature(lang_items)]
#![feature(core_intrinsics)]
#![feature(panic_info_message)]
#![feature(asm)]
#![feature(ptr_internals)]
#![feature(const_fn)]
#![feature(maybe_uninit)]
#![feature(naked_functions)]
#![feature(stmt_expr_attributes)]
#![feature(const_slice_len)]

#[cfg(not(test))]
use core::{
    intrinsics::{likely, unlikely},
    mem,
    panic::PanicInfo,
    ptr,
};
#[cfg(test)]
use std::{
    intrinsics::{likely, unlikely},
    mem,
    panic::PanicInfo,
    ptr,
};

const EXTRA_CHECKS: bool = cfg!(test);

macro_rules! sizeof {
    ($t:ty) => {{
        ::core::mem::size_of::<$t>()
    }};
}

macro_rules! panic_indicator {
    ($x:expr) => ({
        asm!(concat!("mov eax, ", stringify!($x), "; mov [0xb809c], eax") ::: "eax", "memory" : "volatile", "intel");
    });
    () => ({
        panic_indicator!(0x4f214f70);   // !p
    });
}

#[repr(transparent)]
struct SymTable([u32; 0x100]);
impl SymTable {
    /// Reserved in Plan.md
    pub const ADDR: usize = 0x4000;

    /// Unsafe: Must be called only once
    #[inline]
    unsafe fn getmut() -> &'static mut Self {
        &mut *(Self::ADDR as *mut Self)
    }

    #[inline]
    fn init(&mut self) {
        // Init symbol table to zeros
        unsafe {
            ptr::write_bytes(Self::ADDR as *mut u32, 0, 0x100);
        }
    }

    #[inline]
    fn set(&mut self, index: u8, length: u8, value: u32) {
        self.0[index as usize] = value | ((length as u32) << 24);
    }

    #[inline]
    fn cmp(&self, index: u8, length: u8, value: u32) -> bool {
        self.0[index as usize] == value | ((length as u32) << 24)
    }

    /// Private mock constructor for unit testing
    fn mock_new() -> Self {
        Self([0; 0x100])
    }
}

// Keep in sync with src/asm_routines/constants.asm
const ELF_LOADPOINT: usize = 0x10_000;

/// Write 'ER:_' to top top left of screen in red
/// _ denotes the argument
#[cfg(not(test))]
fn error(c: char) -> ! {
    unsafe {
        // 'ER: _'
        asm!("mov rax, 0x4f5f4f3a4f524f45; mov [0xb8000], rax" ::: "rax", "memory" : "volatile", "intel");
        // set arg
        asm!("mov [0xb8008], al" :: "{al}"(c as u8) : "al", "memory" : "volatile", "intel");
        asm!("hlt" :::: "volatile", "intel");
        core::hint::unreachable_unchecked();
    }
}

/// Test error
#[cfg(test)]
fn error(a: char) -> ! {
    panic!("ERR: {}", a);
}

/// Write 'Decompressing: _' on the top left
#[cfg(not(test))]
fn create_progress_indicator() {
    unsafe {
        asm!("mov dword ptr [0xb8000], 0x0f650f44" ::: "memory" : "volatile", "intel");
        asm!("mov dword ptr [0xb8004], 0x0f6f0f63" ::: "memory" : "volatile", "intel");
        asm!("mov dword ptr [0xb8008], 0x0f700f6d" ::: "memory" : "volatile", "intel");
        asm!("mov dword ptr [0xb800c], 0x0f650f72" ::: "memory" : "volatile", "intel");
        asm!("mov dword ptr [0xb8010], 0x0f730f73" ::: "memory" : "volatile", "intel");
        asm!("mov dword ptr [0xb8014], 0x0f6e0f69" ::: "memory" : "volatile", "intel");
        asm!("mov dword ptr [0xb8018], 0x0f3a0f67" ::: "memory" : "volatile", "intel");
        asm!("mov dword ptr [0xb801c], 0x0f5f0f20" ::: "memory" : "volatile", "intel");
    }
}

/// Update 'Decompressing: _' on the top left, _ is the argument
#[cfg(not(test))]
fn progress_indicator(a: u8) {
    unsafe {
        asm!("mov [0xb801e], al" :: "{al}"(a) : "al", "memory" : "volatile", "intel");
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
    if unlikely(ptr::read(ELF_LOADPOINT as *const u32) != 0x464c457f) {
        error('M');
    }

    // Check that the kernel entry point is correct (0x_00000000_00100000, 1MiB)
    if EXTRA_CHECKS && unlikely(ptr::read((ELF_LOADPOINT + 0x18) as *const u64) != 0x100_000) {
        error('P');
    }
}

#[inline(always)]
unsafe fn load_decompression_table(sym_table: &mut SymTable, start: *const u8) {
    // Frequency table can just stay in the ELF image, but
    // Symbol table must be decompressed

    // Read bitstream tree
    let tree_bits = start.offset(0x100);
    let mut offset = 0;
    let mut bit_sc = 7;

    let mut value: u32 = 0;
    let mut length: u8 = 0;

    let mut next_bit = || -> bool {
        let b: u8 = ptr::read(tree_bits.offset(offset));
        let bit = b & (1 << bit_sc) != 0;

        if bit_sc == 0 {
            bit_sc = 7;
            offset += 1;
            if EXTRA_CHECKS && unlikely(offset > 64) {
                error('O');
            }
        } else {
            bit_sc -= 1;
        }

        bit
    };

    // Read first symbol
    for i in 0..=0xff {
        progress_indicator((if i % 2 == 0 { 0x21 } else { 0x20 }) as u8); // blink !

        // Read until next set bit
        loop {
            let nb = next_bit();
            value = (value << 1) | (nb as u32);
            length += 1;
            if nb {
                break;
            }
        }

        // Write symbol
        sym_table.set(i, length - 1, value >> 1);

        // Remove trailing ones
        while value & 1 == 1 {
            value = value >> 1;
            length -= 1;
            if length == 0 {
                return;
            }
        }
        // Then go right (replace the last bit (zero) with one)
        value |= 1;
    }

    // Read the whole value, but tree was not over
    if EXTRA_CHECKS {
        error('T');
    }
}

unsafe fn decompress(
    sym_table: &SymTable,
    frq_table: *const u8,
    src: *const u8,
    dst: *mut u8,
    count: usize,
) {
    let mut out_offset = 0;

    // Read bitstream tree
    let mut offset = 0usize;
    let mut bit_sc = 7;

    let mut length: u8 = 0;
    let mut buffer: u32 = 0;
    while offset < count {
        progress_indicator((if offset % 2 == 0 { 0x23 } else { 0x20 }) as u8); // blink #

        let next_bit = {
            let b: u8 = ptr::read(src.offset(offset as isize));
            let bit = b & (1 << bit_sc) == 0; // note the flip

            if bit_sc == 0 {
                bit_sc = 7;
                offset += 1;
            } else {
                bit_sc -= 1;
            }
            bit
        };

        buffer = (buffer << 1) | (next_bit as u32);
        length += 1;

        if length == 0 {
            break;
        }

        if EXTRA_CHECKS && unlikely(length >= 20) {
            error('L');
        }

        for index in 0u8..=0xff {
            if sym_table.cmp(index, length, buffer) {
                // Matching symbol found
                // Map through frequency table and write
                ptr::write(
                    dst.offset(out_offset),
                    ptr::read(frq_table.offset(index as isize)),
                );
                out_offset += 1;
                length = 0;
                buffer = 0;
                break;
            }
        }
    }
}

#[naked]
#[no_mangle]
pub unsafe extern "C" fn d7boot(a: u8) {
    // Keep decompress indicator on second line
    create_progress_indicator();

    check_elf();

    let mut sym_table = SymTable::getmut();
    // The initialization is not needed, as all the values will be overwritten
    // sym_table.init();

    // Go through the program header table
    // Just assume that the header has standard lengths and positions
    let program_header_count: u16 = ptr::read((ELF_LOADPOINT + 0x38) as *const u16);

    // Load decompression table
    let mut frq_table_addr: usize = 0;
    for i in 0..program_header_count {
        let base = ELF_LOADPOINT + 0x40 + i as usize * 0x38;
        let p_type = ptr::read(base as *const u32);
        // OS Specific 0
        if p_type == 0x60000000 {
            let p_offset = ptr::read((base + 0x08) as *const u64);
            frq_table_addr = ELF_LOADPOINT + p_offset as usize;

            // Loading decompression table
            progress_indicator('?' as u8);

            load_decompression_table(&mut sym_table, frq_table_addr as *const u8);
            break;
        }
    }

    // Not found
    if EXTRA_CHECKS && unlikely(frq_table_addr == 0) {
        error('D');
    }

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

            // Decompress p_filesz bytes from p_offset and write result to p_vaddr
            decompress(
                &sym_table,
                frq_table_addr as *const u8,
                (ELF_LOADPOINT as u64 + p_offset) as *const u8,
                p_vaddr as *mut u8,
                p_filesz as usize,
            );
        }
    }

    progress_indicator('K' as u8);
    asm!("push 0x100000; ret" :::: "volatile", "intel");
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

#[cfg(test)]
mod test {
    use super::{decompress, load_decompression_table, SymTable};

    macro_rules! sum(
        ($head:expr) => ($head);
        ($head:tt, $($tail:tt),*) => ($head + sum!(tail));
    );

    macro_rules! init_array(
        ($ty:ty : $val:expr; $len:expr) => (
            {
                let mut array: [$ty; $len] = unsafe { std::mem::uninitialized() };
                for i in array.iter_mut() {
                    unsafe { ::std::ptr::write(i, $val); }
                }
                array
            }
        )
    );

    macro_rules! concat_arrays(
        ($ty:ty : $arr1:expr, $arr2:expr) => (
            {
                let mut array: [$ty; ($arr1).len() + ($arr2).len()] = unsafe { std::mem::uninitialized() };
                for (i, &v) in array.iter_mut().zip(($arr1).iter().chain(($arr2).iter())) {
                    unsafe { ::std::ptr::write(i, v); }
                }
                array
            }
        )
    );

    #[test]
    fn test_load_decompression_table() {
        let correct_table = mock_symtable();
        let mut sym_table = SymTable::mock_new();

        let src_data: [u8; 0x100 + 64] = concat_arrays!(u8: MOCK_FREQTABLE, MOCK_BITTREE);

        unsafe {
            load_decompression_table(&mut sym_table, &src_data as *const u8);
        }

        for i in 0..=0xff {
            let s = sym_table.0[i];

            let mut found = false;
            for i2 in 0..=0xff {
                if correct_table.cmp(i2, (s >> 24) as u8, s & 0xffff) {
                    found = true;
                }
            }

            println!("i = {} [{} | {:016b}]", i, (s >> 24) as u8, s & 0xffff);
            assert!(found);

            // assert_eq!(s >> 24, c >> 24, "Length incorrect");
            // assert_eq!(s & 0xffff, c & 0xffff, "Value incorrect");
            // assert_eq!(s, c, "Extra bits set");
        }
    }

    #[test]
    fn test_decompress() {
        let mut frq_table = [0u8; 0x100];
        for i in 0..=0xff {
            frq_table[i] = i as u8;
        }

        // Bits here in are written as in symbol table, i.e. bits in correct order
        // However, they are bit-flipped for the decoder
        #[rustfmt::skip]
        let mut src_table: [u8; 8] = [
            !0b_10000111, // 8
            !0b_000_10011, // 0, 0, 0, 24
            !0b_000_10011, // 0, 0, 0, 24
            !0b_10101101,  // 47[:7]
            !0b_1_1010111, // 47[7], 48
            !0b_10000111, // 8
            !0b_000_10011, // 0, 0, 0, 24
            !0b_000_10011, // 0, 0, 0, 24
        ];

        let mut dst_table = [0x01u8; 0x10000]; // use ones and not zeros for error detection
        let mut sym_table = mock_symtable();

        unsafe {
            decompress(
                &mut sym_table,
                (&frq_table) as *const _,
                (&mut src_table) as *mut _ as usize as *mut u8,
                (&mut dst_table) as *mut _ as usize as *mut u8,
                src_table.len(),
            );
        }

        // Left index: decompressed data, right index: from symbol table
        assert_eq!(dst_table[0], 8);
        assert_eq!(dst_table[1..=4], [0, 0, 0, 24]);
        assert_eq!(dst_table[5..=8], [0, 0, 0, 24]);
        assert_eq!(dst_table[9..=10], [47, 48]);
        assert_eq!(dst_table[11], 8);
        assert_eq!(dst_table[12..=15], [0, 0, 0, 24]);
        assert_eq!(dst_table[16..=19], [0, 0, 0, 24]);
    }

    #[test]
    fn verify_symtable_cmp() {
        let st = mock_symtable();

        for i in 0..=0xff {
            assert!(st.cmp(i as u8, MOCK_SYMTABLE[i].0, MOCK_SYMTABLE[i].1));
        }
    }

    fn mock_symtable() -> SymTable {
        let mut sym_table = SymTable::mock_new();
        for (i, (l, v)) in MOCK_SYMTABLE.iter().enumerate() {
            sym_table.set(i as u8, *l, *v);
        }
        sym_table
    }

    const MOCK_BITTREE: [u8; 64] = [
        64, 167, 92, 137, 225, 229, 224, 56, 235, 85, 204, 141, 224, 185, 88, 231, 145, 107, 90,
        176, 19, 105, 204, 179, 202, 90, 204, 184, 45, 135, 55, 6, 243, 86, 28, 57, 229, 101, 112,
        22, 229, 102, 195, 102, 108, 174, 14, 60, 103, 194, 54, 177, 226, 198, 217, 99, 193, 105,
        226, 206, 108, 195, 179, 30,
    ];

    const MOCK_FREQTABLE: [u8; 0x100] = [
        0, 114, 99, 209, 176, 56, 65, 128, 192, 72, 36, 53, 162, 161, 204, 5, 158, 90, 197, 11,
        206, 212, 187, 84, 255, 210, 200, 57, 181, 163, 243, 82, 179, 174, 131, 98, 207, 157, 149,
        4, 76, 15, 31, 38, 156, 42, 147, 61, 32, 83, 201, 50, 254, 46, 208, 213, 173, 154, 152, 33,
        245, 34, 225, 137, 141, 14, 228, 143, 66, 35, 122, 48, 88, 231, 136, 140, 69, 109, 104,
        244, 169, 155, 26, 103, 203, 151, 20, 45, 7, 236, 220, 150, 145, 165, 239, 2, 116, 16, 184,
        27, 180, 183, 146, 125, 144, 47, 80, 191, 185, 68, 40, 93, 60, 199, 112, 159, 227, 96, 74,
        241, 172, 134, 73, 25, 217, 214, 164, 133, 8, 235, 142, 64, 251, 218, 160, 193, 95, 97,
        130, 113, 171, 177, 51, 166, 108, 111, 195, 107, 13, 233, 94, 148, 229, 139, 10, 205, 189,
        87, 196, 117, 49, 52, 234, 170, 58, 86, 190, 79, 121, 182, 77, 223, 198, 28, 22, 230, 168,
        81, 102, 54, 43, 29, 138, 85, 67, 132, 242, 39, 91, 24, 246, 118, 62, 135, 247, 232, 1,
        115, 123, 219, 215, 188, 70, 211, 202, 41, 240, 12, 105, 44, 224, 252, 126, 89, 75, 55,
        127, 124, 30, 9, 119, 37, 194, 19, 100, 92, 23, 222, 6, 175, 221, 153, 110, 248, 237, 250,
        59, 238, 21, 78, 17, 186, 253, 3, 101, 216, 226, 71, 129, 18, 63, 120, 178, 106, 249, 167,
    ];

    const MOCK_SYMTABLE: [(u8, u32); 0x100] = [
        (1, 0b0),
        (7, 0b1000000),
        (8, 0b10000010),
        (10, 0b1000001100),
        (10, 0b1000001101),
        (9, 0b100000111),
        (7, 0b1000010),
        (8, 0b10000110),
        (8, 0b10000111),
        (5, 0b10001),
        (6, 0b100100),
        (9, 0b100101000),
        (11, 0b10010100100),
        (11, 0b10010100101),
        (10, 0b1001010011),
        (8, 0b10010101),
        (11, 0b10010110000),
        (11, 0b10010110001),
        (10, 0b1001011001),
        (9, 0b100101101),
        (10, 0b1001011100),
        (11, 0b10010111010),
        (11, 0b10010111011),
        (9, 0b100101111),
        (5, 0b10011),
        (10, 0b1010000000),
        (10, 0b1010000001),
        (9, 0b101000001),
        (11, 0b10100001000),
        (11, 0b10100001001),
        (10, 0b1010000101),
        (10, 0b1010000110),
        (11, 0b10100001110),
        (11, 0b10100001111),
        (8, 0b10100010),
        (9, 0b101000110),
        (10, 0b1010001110),
        (11, 0b10100011110),
        (11, 0b10100011111),
        (6, 0b101001),
        (7, 0b1010100),
        (7, 0b1010101),
        (8, 0b10101100),
        (11, 0b10101101000),
        (11, 0b10101101001),
        (11, 0b10101101010),
        (11, 0b10101101011),
        (9, 0b101011011),
        (7, 0b1010111),
        (9, 0b101100000),
        (10, 0b1011000010),
        (10, 0b1011000011),
        (8, 0b10110001),
        (9, 0b101100100),
        (10, 0b1011001010),
        (11, 0b10110010110),
        (11, 0b10110010111),
        (11, 0b10110011000),
        (11, 0b10110011001),
        (10, 0b1011001101),
        (11, 0b10110011100),
        (11, 0b10110011101),
        (10, 0b1011001111),
        (6, 0b101101),
        (7, 0b1011100),
        (10, 0b1011101000),
        (11, 0b10111010010),
        (11, 0b10111010011),
        (10, 0b1011101010),
        (11, 0b10111010110),
        (11, 0b10111010111),
        (9, 0b101110110),
        (10, 0b1011101110),
        (10, 0b1011101111),
        (7, 0b1011110),
        (8, 0b10111110),
        (9, 0b101111110),
        (9, 0b101111111),
        (9, 0b110000000),
        (11, 0b11000000100),
        (11, 0b11000000101),
        (11, 0b11000000110),
        (11, 0b11000000111),
        (9, 0b110000010),
        (11, 0b11000001100),
        (11, 0b11000001101),
        (10, 0b1100000111),
        (9, 0b110000100),
        (9, 0b110000101),
        (10, 0b1100001100),
        (11, 0b11000011010),
        (11, 0b11000011011),
        (11, 0b11000011100),
        (11, 0b11000011101),
        (10, 0b1100001111),
        (6, 0b110001),
        (7, 0b1100100),
        (8, 0b11001010),
        (10, 0b1100101100),
        (11, 0b11001011010),
        (11, 0b11001011011),
        (10, 0b1100101110),
        (11, 0b11001011110),
        (11, 0b11001011111),
        (8, 0b11001100),
        (8, 0b11001101),
        (9, 0b110011100),
        (10, 0b1100111010),
        (10, 0b1100111011),
        (8, 0b11001111),
        (9, 0b110100000),
        (10, 0b1101000010),
        (10, 0b1101000011),
        (9, 0b110100010),
        (9, 0b110100011),
        (11, 0b11010010000),
        (11, 0b11010010001),
        (10, 0b1101001001),
        (11, 0b11010010100),
        (11, 0b11010010101),
        (11, 0b11010010110),
        (11, 0b11010010111),
        (8, 0b11010011),
        (11, 0b11010100000),
        (11, 0b11010100001),
        (11, 0b11010100010),
        (11, 0b11010100011),
        (9, 0b110101001),
        (8, 0b11010101),
        (9, 0b110101100),
        (9, 0b110101101),
        (9, 0b110101110),
        (10, 0b1101011110),
        (11, 0b11010111110),
        (11, 0b11010111111),
        (9, 0b110110000),
        (9, 0b110110001),
        (8, 0b11011001),
        (11, 0b11011010000),
        (11, 0b11011010001),
        (10, 0b1101101001),
        (11, 0b11011010100),
        (11, 0b11011010101),
        (10, 0b1101101011),
        (8, 0b11011011),
        (8, 0b11011100),
        (9, 0b110111010),
        (10, 0b1101110110),
        (10, 0b1101110111),
        (9, 0b110111100),
        (10, 0b1101111010),
        (11, 0b11011110110),
        (11, 0b11011110111),
        (8, 0b11011111),
        (10, 0b1110000000),
        (11, 0b11100000010),
        (11, 0b11100000011),
        (10, 0b1110000010),
        (10, 0b1110000011),
        (8, 0b11100001),
        (9, 0b111000100),
        (10, 0b1110001010),
        (11, 0b11100010110),
        (11, 0b11100010111),
        (10, 0b1110001100),
        (10, 0b1110001101),
        (10, 0b1110001110),
        (10, 0b1110001111),
        (10, 0b1110010000),
        (10, 0b1110010001),
        (10, 0b1110010010),
        (10, 0b1110010011),
        (10, 0b1110010100),
        (10, 0b1110010101),
        (11, 0b11100101100),
        (11, 0b11100101101),
        (11, 0b11100101110),
        (11, 0b11100101111),
        (9, 0b111001100),
        (10, 0b1110011010),
        (11, 0b11100110110),
        (11, 0b11100110111),
        (8, 0b11100111),
        (10, 0b1110100000),
        (10, 0b1110100001),
        (9, 0b111010001),
        (11, 0b11101001000),
        (11, 0b11101001001),
        (10, 0b1110100101),
        (9, 0b111010011),
        (10, 0b1110101000),
        (10, 0b1110101001),
        (11, 0b11101010100),
        (11, 0b11101010101),
        (10, 0b1110101011),
        (8, 0b11101011),
        (6, 0b111011),
        (8, 0b11110000),
        (11, 0b11110001000),
        (11, 0b11110001001),
        (11, 0b11110001010),
        (11, 0b11110001011),
        (10, 0b1111000110),
        (11, 0b11110001110),
        (11, 0b11110001111),
        (10, 0b1111001000),
        (10, 0b1111001001),
        (9, 0b111100101),
        (8, 0b11110011),
        (9, 0b111101000),
        (10, 0b1111010010),
        (10, 0b1111010011),
        (11, 0b11110101000),
        (11, 0b11110101001),
        (11, 0b11110101010),
        (11, 0b11110101011),
        (10, 0b1111010110),
        (10, 0b1111010111),
        (9, 0b111101100),
        (10, 0b1111011010),
        (10, 0b1111011011),
        (11, 0b11110111000),
        (11, 0b11110111001),
        (10, 0b1111011101),
        (9, 0b111101111),
        (10, 0b1111100000),
        (11, 0b11111000010),
        (11, 0b11111000011),
        (10, 0b1111100010),
        (12, 0b111110001100),
        (12, 0b111110001101),
        (11, 0b11111000111),
        (8, 0b11111001),
        (10, 0b1111101000),
        (11, 0b11111010010),
        (11, 0b11111010011),
        (11, 0b11111010100),
        (11, 0b11111010101),
        (10, 0b1111101011),
        (10, 0b1111101100),
        (10, 0b1111101101),
        (10, 0b1111101110),
        (10, 0b1111101111),
        (8, 0b11111100),
        (8, 0b11111101),
        (11, 0b11111110000),
        (11, 0b11111110001),
        (10, 0b1111111001),
        (10, 0b1111111010),
        (10, 0b1111111011),
        (10, 0b1111111100),
        (10, 0b1111111101),
        (12, 0b111111111000),
        (12, 0b111111111001),
        (11, 0b11111111101),
        (10, 0b1111111111),
    ];
}
