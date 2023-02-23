// Code style
#![forbid(private_in_public)]
#![deny(unused_assignments)]
// Safety
#![deny(overflowing_literals)]
#![deny(unused_must_use)]
// Clippy
#![warn(clippy::all)]

use std::env;
use std::fs::File;
use std::io::{prelude::*, BufReader, SeekFrom};
use std::mem::size_of;

use num_traits::{
    cast::{NumCast, ToPrimitive},
    int::PrimInt,
    sign::Unsigned,
};

/// Read integer at
fn read_int<T: PrimInt + Unsigned + ToPrimitive + NumCast>(f: &mut File) -> T {
    let mut result: T = T::zero();

    let mut bytes = f
        .bytes()
        .take(size_of::<T>())
        .map(Some)
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

#[allow(dead_code)]
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

    if args.len() != 2 {
        println!("usage: elf2bin in_elf out_elf");
        return;
    }

    let in_path = &args[0];
    let out_path = &args[1];

    let mut f = File::open(in_path)
        .unwrap_or_else(|err| panic!("Input file not found ({}) ({})", in_path, err));

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

    assert_eq!(
        p_headers.len(),
        1,
        "Only one loadable program header must be present"
    );
    let p_header = p_headers[0];

    // Load program parts to memory
    // Use buffered reader for speed
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

    // Write new file, from scratch
    let mut outf = File::create(out_path)
        .unwrap_or_else(|err| panic!("Could not create output file ({}) ({})", out_path, err));
    outf.write_all(&data.as_slice()).unwrap();
}
