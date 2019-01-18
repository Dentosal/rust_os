// Code style
#![forbid(private_in_public)]
#![deny(unused_assignments)]
// Safety
#![deny(overflowing_literals)]
#![deny(safe_packed_borrows)]
#![deny(unused_must_use)]
// Features
#![feature(reverse_bits)]

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{prelude::*, BufReader, SeekFrom};
use std::iter::FromIterator;
use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};

use huffman_compress::CodeBuilder;

use num_traits::{
    cast::{NumCast, ToPrimitive},
    int::PrimInt,
    sign::Unsigned,
};

use rayon::prelude::*;

use d7elfpack::*;

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

/// Read integer at
fn read_int<T: PrimInt + Unsigned + ToPrimitive + NumCast>(f: &mut File) -> T {
    let mut result: T = T::zero();

    let mut bytes = f
        .bytes()
        .take(size_of::<T>())
        .map(|x| Some(x))
        .chain(None)
        .collect::<Vec<_>>();

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
    for (a, t) in f.bytes().map(|x| Some(x)).chain(None).zip(target.iter()) {
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

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.len() == 0 {
        println!("usage: in_elf out_elf");
        return;
    }

    let in_path = &args[0];
    let out_path = &args[1];

    let mut f = File::open(in_path).expect(&format!("Input file not found ({})", in_path));

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

    let byte_counts = init_array![AtomicUsize: AtomicUsize::new(0); 0x100];
    program_parts.par_iter().for_each(|pb| {
        pb.par_iter().for_each(|byte| {
            byte_counts[(*byte) as usize].fetch_add(1, Ordering::Relaxed);
        });
    });

    // Make sure there it at least one of each byte, so the Hufman symbol tree size,
    // doesn't have to be stored, as it is always 256 elements. This is slightly
    // suboptimal if we don't have all bytes, but it doesn't really matter
    let mut weights: HashMap<u8, usize> = HashMap::new();
    print!("Weights: [");
    for byte in 0..=0xff {
        print!("{}", byte_counts[byte as usize].load(Ordering::Acquire).max(1));
        if byte != 0xff {
            print!(", ");
        }
        weights.insert(byte, byte_counts[byte as usize].load(Ordering::Acquire).max(1));
    }
    println!("]");

    let (book, tree) = CodeBuilder::from_iter(weights).finish();
    assert_eq!(book.len(), 0x100);

    println!(
        "Original_start: {:?}",
        program_parts[0].iter().cloned().take(100).collect::<Vec<u8>>()
    );

    program_parts = program_parts
        .iter()
        .cloned()
        .map(|p| compress(book.clone(), p))
        .collect();

    println!("{:?}", book);

    let (bittree, frq_table) = tree.to_bits();
    assert_eq!(bittree.len(), 511);

    println!("BitTree: {:?}", bittree);
    println!("BitTree bytes: {:?}", bittree.to_bytes());

    println!("FrqTable: {:?}", frq_table);

    println!(
        "Compressed start: {:?}",
        program_parts[0].iter().cloned().take(100).collect::<Vec<u8>>()
    );

    // Write new file, from scratch
    let mut outf = File::create(out_path).expect(&format!("Could not create output file ({})", out_path));

    // https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#File_header
    #[rustfmt::skip]
    let file_header: [u8; 0x40] = [
        // Magic
        0x7f,
        'E' as u8,
        'L' as u8,
        'F' as u8,
        // Class: 64bit, Data: little-endian, Version: 1, ABI: System V
        2, 1, 1, 0,
        // ABI version (meaning not defined), 7x padding
        0, 0, 0, 0, 0, 0, 0, 0,
        // Type: EXEC
        0x02, 0x00,
        // Instruction set: x64-64
        0x3e, 0x00,
        // Version: 1
        1, 0, 0, 0,
        // Entry point: 0x100_000
        0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00,
        // Program header offset: 0x40 (Just after the file header)
        0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        // Section header offset: 0x00 (unused)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        // Flags: None
        0, 0, 0, 0,
        // Size of this header
        0x40, 0x00,
        // Program header entry size: 0x38 bytes (56 bytes, default)
        0x38, 0x00,
        // Program header entry count
        (program_parts.len() + 1) as u8,
        0x00,
        // Section header entry size: 0x40 bytes (64 bytes, default)
        0x40, 0x00,
        // Section header entry count: no entries
        0x00, 0x00,
        // Section header entry containing section names: 0x00 (unused)
        0x00, 0x00,
    ];

    outf.write_all(&file_header).unwrap();

    // Program headers
    // https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#Program_header
    let mut offset: u64 = 0x40 + (p_headers.len() as u64 + 1) * 0x38;
    for (ph, pp) in p_headers.iter().zip(program_parts.iter()) {
        // Segment type: 1 PT_LOAD
        outf.write_all(&1u32.to_le_bytes()).unwrap();
        // Flags: keep original
        outf.write_all(&ph.flags.to_le_bytes()).unwrap();
        // Offset: using offset counter
        outf.write_all(&offset.to_le_bytes()).unwrap();
        // Virtual address: Keep original
        outf.write_all(&ph.vaddr.to_le_bytes()).unwrap();
        // Physical address: Keep original
        outf.write_all(&ph.paddr.to_le_bytes()).unwrap();
        // Size in ELF file: Use compressed size
        outf.write_all(&(pp.len() as u64).to_le_bytes()).unwrap();
        // Size in memory after loading: Keep original
        outf.write_all(&ph.memsz.to_le_bytes()).unwrap();
        // Align (keep original)
        outf.write_all(&ph.align.to_le_bytes()).unwrap();

        offset += pp.len() as u64;
    }

    // One more program header, containing info for the decompression tables

    // Segment type: 0x60000000 OS Specific (first available)
    outf.write_all(&0x60000000u32.to_le_bytes()).unwrap();
    // Flags: No
    outf.write_all(&0u32.to_le_bytes()).unwrap();
    // Offset: using offset counter
    outf.write_all(&offset.to_le_bytes()).unwrap();
    // Virtual address: Unused
    outf.write_all(&0u64.to_le_bytes()).unwrap();
    // Physical address: Unused
    outf.write_all(&0u64.to_le_bytes()).unwrap();
    // Size in ELF file: (320) (256 for data bytes and 64 for bit tree)
    outf.write_all(&320u64.to_le_bytes()).unwrap();
    // Size in memory after loading: Unused
    outf.write_all(&0u64.to_le_bytes()).unwrap();
    // Align (keep original)
    outf.write_all(&0u64.to_le_bytes()).unwrap();
    // offset += 320;

    // Program parts
    for pp in program_parts {
        // let qq: Vec<u8> = pp.iter().map(|p| p.reverse_bits()).collect();
        // let qq: Vec<u8> = pp.iter().map(|p| !p).collect();
        // outf.write_all(&qq.as_slice()).unwrap();
        outf.write_all(&pp.as_slice()).unwrap();
    }

    // Decompression table
    outf.write_all(&frq_table.as_slice()).unwrap();
    outf.write_all(&bittree.to_bytes()).unwrap();
}
