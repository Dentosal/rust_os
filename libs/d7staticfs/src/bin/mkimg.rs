#![deny(unused_must_use)]

extern crate d7staticfs;

use std::env;
use std::fs::{File, OpenOptions};
use std::io::{prelude::*, SeekFrom};
use std::u32;

use d7staticfs::*;

fn round_up_sector(p: u64) -> u64 {
    (p + SECTOR_SIZE - 1) / SECTOR_SIZE
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.len() == 0 {
        println!("usage: disk.img kernel_skip_index [filename=filepath ...]");
        return;
    }

    let disk_img_path = &args[0];
    let kernel_skip_index = args[1]
        .parse::<u32>()
        .expect("kernel_skip_index: Integer required");

    let mut files: Vec<(String, FileEntry, u32)> = Vec::new();
    for filearg in args.iter().skip(2) {
        let halfs = filearg.splitn(2, '=').collect::<Vec<_>>();
        assert_eq!(halfs.len(), 2);

        let fs_filename = halfs[0].trim();
        let rl_filename = halfs[1].trim();

        for (_, e, _) in &files {
            assert!(!e.name_matches(fs_filename), "Duplicate names not allowed");
        }

        let f = File::open(rl_filename).expect(&format!("File not found: {:?}", rl_filename));
        let meta = f.metadata().expect("Metadata not found");
        assert!(meta.is_file());

        let size_sectors = round_up_sector(meta.len());
        assert!(size_sectors <= u32::MAX as u64);

        let fe = FileEntry::new(fs_filename, size_sectors as u32);
        files.push((rl_filename.to_owned(), fe, 0));
    }

    let meta = {
        let f = File::open(disk_img_path).expect("Target file not found");
        f.metadata().expect("Target file metadata not available")
    };
    let size_sectors = round_up_sector(meta.len());
    assert!(size_sectors <= u32::MAX as u64);
    assert_eq!(meta.len(), size_sectors * SECTOR_SIZE);

    // Check that the file is big enough
    // TODO: lt or lte?
    let required_size_sectors = kernel_skip_index as u64
        + round_up_sector(16 + files.len() as u64 * 16)
        + files.iter().map(|(_, e, _)| e.size as u64).sum::<u64>();
    assert!(
        required_size_sectors < size_sectors,
        "File is not large enough, sectors required: {}, got only {}",
        required_size_sectors,
        size_sectors
    );

    {
        // Check placeholder magic
        let mut f = File::open(disk_img_path).expect("Target file not found");
        let mut magic_check: [u8; 4] = [0; 4];
        f.seek(SeekFrom::Start(MBR_POSITION as u64)).unwrap();
        f.read(&mut magic_check).unwrap();
        assert_eq!(
            magic_check,
            HEADER_MAGIC.to_le_bytes(),
            "Magic placeholder missing"
        );
    }

    {
        // Check kernel skip index
        let mut f = File::open(disk_img_path).expect("Target file not found");
        let mut empty_check: [u8; 4] = [0; 4];
        f.seek(SeekFrom::Start((kernel_skip_index as u64) * SECTOR_SIZE))
            .unwrap();
        f.read(&mut empty_check).unwrap();
        assert_eq!(empty_check, [0; 4]);
    }

    {
        // Set LBA pointer
        let mut f = OpenOptions::new() // overwrite, don't insert in middle
            .read(false)
            .write(true)
            .create(false)
            .open(disk_img_path)
            .unwrap();

        let mut lba_ptr: [u8; 4] = (kernel_skip_index as u32).to_le_bytes();
        f.seek(SeekFrom::Start(MBR_POSITION as u64)).unwrap();
        f.write_all(&mut lba_ptr).unwrap();
    }

    {
        // Write file table and file contents
        let mut f = OpenOptions::new() // overwrite, don't insert in middle
            .read(false)
            .write(true)
            .create(false)
            .open(disk_img_path)
            .unwrap();

        let header_0: [u8; 4] = HEADER_MAGIC.to_le_bytes();
        let header_1: [u8; 4] = ONLY_VERSION.to_le_bytes();
        let header_2: [u8; 4] = (files.len() as u32).to_le_bytes();
        let header_3: [u8; 4] = 0u32.to_le_bytes();
        f.seek(SeekFrom::Start(kernel_skip_index as u64 * SECTOR_SIZE))
            .unwrap();

        // Header
        f.write_all(&header_0).unwrap();
        f.write_all(&header_1).unwrap();
        f.write_all(&header_2).unwrap();
        f.write_all(&header_3).unwrap();

        // Entries
        for (_, entry, _) in &files {
            f.write_all(&entry.to_bytes()).unwrap();
        }

        // Align to sector boundary
        let files_start_pos =
            kernel_skip_index as u64 + round_up_sector(16 + files.len() as u64 * 16);

        f.seek(SeekFrom::Start(files_start_pos * SECTOR_SIZE))
            .unwrap();
        let mut files_sectors_count: u64 = 0;

        // Copy files to disk image
        for (i, (path, entry, _)) in files.clone().iter().enumerate() {
            let mut rf = File::open(path).expect(&format!("Source file '{}' not found", path));

            files[i].2 = files_sectors_count as u32;

            let mut counter: u64 = 0;
            let mut buffer = [0u8; 0x200];
            loop {
                let bytes_read = rf.read(&mut buffer).unwrap();
                if bytes_read == 0 {
                    break;
                }
                counter += bytes_read as u64;
                f.write_all(&buffer[..bytes_read]).unwrap();
            }

            // Zero-pad to sector boundary
            f.write_all(&vec![0; (SECTOR_SIZE - (counter % SECTOR_SIZE)) as usize].as_slice())
                .unwrap();

            assert_eq!(round_up_sector(counter), entry.size as u64);
            files_sectors_count += entry.size as u64;
        }

        assert_eq!(
            files_sectors_count,
            files.iter().map(|(_, e, _)| e.size as u64).sum::<u64>()
        );
    }

    println!(" File Name    | Offset (hex) | Size (hex) | Host Path ");
    println!("--------------|--------------|------------|-----------");
    for (host_path, file, offset) in files {
        println!(
            " {:<12} |     {:>8x} |   {:>8x} | {}",
            std::str::from_utf8(&file.name).unwrap(),
            offset,
            file.size,
            host_path
        );
    }
    println!("");
}
