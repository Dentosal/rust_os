// Code style
#![forbid(private_in_public)]
// Safety
#![deny(overflowing_literals)]
#![deny(safe_packed_borrows)]
#![deny(unused_must_use)]
// Features
#![feature(maybe_uninit_extra)]

use std::env;
use std::fs::File;
use std::io::{prelude::*, BufReader, SeekFrom};
use std::mem::size_of;

use bit_vec::BitVec;

use num_traits::{
    cast::{NumCast, ToPrimitive},
    int::PrimInt,
    sign::Unsigned,
};

use d7elfpack::*;

/// Read integer at
fn read_int<T: PrimInt + Unsigned + ToPrimitive + NumCast>(f: &mut File) -> T {
    let mut result: T = T::zero();

    let mut bytes = f.bytes().take(size_of::<T>()).map(Some).chain(None).collect::<Vec<_>>();

    bytes.reverse();

    for a in bytes {
        if let Some(b) = a {
            result = (result << 8) | T::from(b.unwrap()).unwrap();
        } else {
            panic!("File was not long enough (read_int)");
        }
    }
    result
}
fn read_int_at<T: PrimInt + Unsigned + ToPrimitive + NumCast>(f: &mut File, pos: u64) -> T {
    f.seek(SeekFrom::Start(pos)).unwrap();
    read_int::<T>(f)
}

fn check_at_eq(f: &mut File, pos: u64, target: &[u8], msg: &str) {
    f.seek(SeekFrom::Start(pos)).unwrap();
    for (a, t) in f.bytes().map(Some).chain(None).zip(target.iter()) {
        if let Some(b) = a {
            assert_eq!(&b.unwrap(), t, "Byte mismatch in {}", msg);
        } else {
            panic!("File was not long enough (in {} test)", msg);
        }
    }
}

/// Check that this is a valid ELF file
/// https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#File_header
fn check_elf(f: &mut File) {
    check_at_eq(f, 0, &[0x7f, 0x45, 0x4c, 0x46], "magic");
    check_at_eq(f, 4, &[0x02], "bitness");
    check_at_eq(f, 5, &[0x01], "endianess");
    check_at_eq(f, 6, &[0x01], "version");
    check_at_eq(f, 18, &[0x3e], "instruction set");
    check_at_eq(f, 54, &[0x38], "program header size");
}

#[derive(Debug, Copy, Clone)]
struct ProgramHeader {
    segment_type: u32,
    flags: u32,
    offset: u64,
    vaddr: u64,
    paddr: u64,
    filesz: u64,
    memsz: u64,
    align: u64,
}

fn load_original(in_path: &str) -> Vec<Vec<u8>> {
    let mut f = File::open(in_path).unwrap_or_else(|_| panic!("Input file not found ({})", in_path));

    check_elf(&mut f);

    // Program header table
    let pht_pos = read_int_at::<u64>(&mut f, 32);
    let pht_len = read_int_at::<u16>(&mut f, 56);

    assert!(pht_len > 0, "Empty program header table");

    f.seek(SeekFrom::Start(pht_pos)).unwrap();

    // https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#Program_header
    let mut p_headers: Vec<ProgramHeader> = Vec::new();
    for _ in 0..pht_len {
        let segment_type = read_int::<u32>(&mut f);
        if segment_type == 1 {
            // LOAD segment
            let flags = read_int::<u32>(&mut f);
            let offset = read_int::<u64>(&mut f);
            let vaddr = read_int::<u64>(&mut f);
            let paddr = read_int::<u64>(&mut f);
            let filesz = read_int::<u64>(&mut f);
            let memsz = read_int::<u64>(&mut f);
            let align = read_int::<u64>(&mut f);

            p_headers.push(ProgramHeader {
                segment_type,
                flags,
                offset,
                vaddr,
                paddr,
                filesz,
                memsz,
                align,
            });
        }
    }

    // Load program parts to memory
    let mut program_parts: Vec<Vec<u8>> = Vec::new();
    for p_header in &p_headers {
        // Use buffered reader for (measured) 200x speedup

        let mut fbuf = File::open(in_path).unwrap();
        fbuf.seek(SeekFrom::Start(p_header.offset)).unwrap();
        let reader = BufReader::new(fbuf);

        let data: Vec<u8> = reader
            .bytes()
            .take(p_header.filesz as usize)
            .map(Result::unwrap)
            .collect();

        assert_eq!(
            data.len(),
            p_header.filesz as usize,
            "File was too small (incorrect offset or filesz)"
        );

        program_parts.push(data);
    }

    assert!(program_parts.len() < 0xff);
    program_parts
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.len() != 2 {
        println!("usage: original_elf compressed_elf");
        return;
    }

    let original_path = &args[0];
    let compressed_path = &args[1];

    let mut f = File::open(compressed_path).unwrap_or_else(|_| panic!("Input file not found ({})", compressed_path));

    check_elf(&mut f);

    // Program header table
    let pht_pos = read_int_at::<u64>(&mut f, 32);
    let pht_len = read_int_at::<u16>(&mut f, 56);

    assert!(pht_len > 0, "Empty program header table");

    f.seek(SeekFrom::Start(pht_pos)).unwrap();

    let mut p_headers: Vec<ProgramHeader> = Vec::new();
    let mut compress_table_offset: Option<u64> = None;
    for _ in 0..pht_len {
        let segment_type = read_int::<u32>(&mut f);
        if segment_type == 1 {
            // LOAD segment
            let flags = read_int::<u32>(&mut f);
            let offset = read_int::<u64>(&mut f);
            let vaddr = read_int::<u64>(&mut f);
            let paddr = read_int::<u64>(&mut f);
            let filesz = read_int::<u64>(&mut f);
            let memsz = read_int::<u64>(&mut f);
            let align = read_int::<u64>(&mut f);

            p_headers.push(ProgramHeader {
                segment_type,
                flags,
                offset,
                vaddr,
                paddr,
                filesz,
                memsz,
                align,
            });
        } else if segment_type == 0x6000_0000u32 {
            // Segment type: 0x60000000 OS Specific
            let flags = read_int::<u32>(&mut f);
            let offset = read_int::<u64>(&mut f);
            let vaddr = read_int::<u64>(&mut f);
            let paddr = read_int::<u64>(&mut f);
            let filesz = read_int::<u64>(&mut f);
            let memsz = read_int::<u64>(&mut f);
            let align = read_int::<u64>(&mut f);

            assert_eq!(flags, 0);
            assert_eq!(vaddr, 0);
            assert_eq!(paddr, 0);
            assert_eq!(filesz, 320);
            assert_eq!(memsz, 0);
            assert_eq!(align, 0);

            compress_table_offset = Some(offset);
        }
    }

    // Load program parts to memory
    let mut program_parts: Vec<Vec<u8>> = Vec::new();
    for p_header in &p_headers {
        // Use buffered reader for (measured) 200x speedup

        let mut fbuf = File::open(compressed_path).unwrap();
        fbuf.seek(SeekFrom::Start(p_header.offset)).unwrap();
        let reader = BufReader::new(fbuf);

        let data: Vec<u8> = reader
            .bytes()
            .take(p_header.filesz as usize)
            .map(Result::unwrap)
            .collect();

        assert_eq!(
            data.len(),
            p_header.filesz as usize,
            "File was too small (incorrect offset or filesz)"
        );

        program_parts.push(data);
    }

    assert!(program_parts.len() < 0xff);

    let ct_offset = compress_table_offset.unwrap();

    f.seek(SeekFrom::Start(ct_offset)).unwrap();
    let frqs: Vec<u8> = (0..0x100).map(|_| read_int::<u8>(&mut f)).collect();
    let bytes: Vec<u8> = (0..64).map(|_| read_int::<u8>(&mut f)).collect();
    let mut bits = BitVec::from_bytes(&bytes);
    let extra_bit = bits.pop();
    assert_eq!(extra_bit, Some(false));
    assert_eq!(bits.len(), 511);

    let original_parts = load_original(original_path);
    for (pp, original_pp) in program_parts.into_iter().zip(original_parts.into_iter()) {
        let decompressed: Vec<u8> = decompress(bits.clone(), &frqs, &pp);
        assert_eq!(decompressed.len(), original_pp.len());
        assert_eq!(decompressed, original_pp);
    }
}
