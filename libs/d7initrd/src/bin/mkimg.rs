#![deny(unused_must_use)]

extern crate d7initrd;

use std::env;
use std::fs::{File, OpenOptions};
use std::io::{prelude::*, SeekFrom};
use std::u64;

use d7initrd::*;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        println!("usage: disk.img kernel_skip_index [filename=filepath ...]");
        return;
    }

    let disk_img_path = &args[0];
    let kernel_skip_index = args[1]
        .parse::<u64>()
        .expect("kernel_skip_index: Integer required");

    let mut cumulative_offset = 0;
    let mut files: Vec<(String, FileEntry)> = Vec::new();
    for filearg in args.iter().skip(2) {
        let halfs = filearg.splitn(2, '=').collect::<Vec<_>>();
        assert_eq!(halfs.len(), 2);

        let fs_filename = halfs[0].trim();
        let rl_filename = halfs[1].trim();

        let f =
            File::open(rl_filename).unwrap_or_else(|_| panic!("File not found: {:?}", rl_filename));
        let meta = f.metadata().expect("Metadata not found");
        assert!(meta.is_file());

        let fe = FileEntry {
            name: fs_filename.to_owned(),
            size: meta.len(),
            offset: cumulative_offset,
        };
        files.push((rl_filename.to_owned(), fe));
        cumulative_offset += meta.len();
    }

    let meta = {
        let f = File::open(disk_img_path).expect("Target file not found");
        f.metadata().expect("Target file metadata not available")
    };
    let size_sectors = meta.len() / SECTOR_SIZE;

    // Prepare header
    let header_body_contents: Vec<FileEntry> = files.iter().map(|(_, e)| e.clone()).collect();
    let header_body: Vec<u8> = pinecone::to_vec(&header_body_contents).unwrap();
    let header_size = HEADER_SIZE_BYTES + header_body.len();
    let files_size: usize = files.iter().map(|(_, e)| e.size as usize).sum();

    let required_size_sectors =
        kernel_skip_index as u64 + to_sectors_round_up((header_size + files_size) as u64);
    assert!(
        required_size_sectors < size_sectors, // TODO: lt or lte?
        "File is not large enough, sectors required: {}, got only {}",
        required_size_sectors,
        size_sectors
    );

    {
        // Check placeholder magic
        let mut f = File::open(disk_img_path).expect("Target file not found");
        let mut magic_check: [u8; 4] = [0; 4];
        f.seek(SeekFrom::Start(MBR_POSITION_S as u64)).unwrap();
        f.read_exact(&mut magic_check).unwrap();
        assert_eq!(
            magic_check,
            HEADER_MAGIC.to_le_bytes(),
            "Magic placeholder 1 missing from the target image"
        );
        f.seek(SeekFrom::Start(MBR_POSITION_E as u64)).unwrap();
        f.read_exact(&mut magic_check).unwrap();
        assert_eq!(
            magic_check,
            HEADER_MAGIC.to_le_bytes(),
            "Magic placeholder 2 missing from the target image"
        );
    }

    {
        // Check kernel skip index
        let mut f = File::open(disk_img_path).expect("Target file not found");
        let mut empty_check: [u8; 4] = [0; 4];
        f.seek(SeekFrom::Start((kernel_skip_index as u64) * SECTOR_SIZE))
            .unwrap();
        f.read_exact(&mut empty_check).unwrap();
        assert_eq!(empty_check, [0; 4]);
    }

    {
        // Set LBA pointers
        let mut f = OpenOptions::new() // overwrite, don't insert in middle
            .read(false)
            .write(true)
            .create(false)
            .open(disk_img_path)
            .unwrap();

        let lba_ptr_split: [u8; 4] = (kernel_skip_index as u32).to_le_bytes();
        let lba_ptr_end: [u8; 4] = (required_size_sectors as u32).to_le_bytes();

        f.seek(SeekFrom::Start(MBR_POSITION_S as u64)).unwrap();
        f.write_all(&lba_ptr_split).unwrap();

        f.seek(SeekFrom::Start(MBR_POSITION_E as u64)).unwrap();
        f.write_all(&lba_ptr_end).unwrap();
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
        let header_1: [u8; 4] = (header_body.len() as u32).to_le_bytes();
        let header_2: [u8; 8] = ((header_size + files_size) as u64).to_le_bytes();

        f.seek(SeekFrom::Start(kernel_skip_index as u64 * SECTOR_SIZE))
            .unwrap();

        // Header
        f.write_all(&header_0).unwrap();
        f.write_all(&header_1).unwrap();
        f.write_all(&header_2).unwrap();

        // Header body
        f.write_all(&header_body).unwrap();

        // Copy files to the disk image
        for (path, _entry) in &files {
            let mut rf =
                File::open(path).unwrap_or_else(|_| panic!("Source file '{}' not found", path));

            let mut buffer = [0u8; 0x200];
            loop {
                let bytes_read = rf.read(&mut buffer).unwrap();
                if bytes_read == 0 {
                    break;
                }
                f.write_all(&buffer[..bytes_read]).unwrap();
            }
        }
    }

    println!(" File Name                      | Size (hex) | Host Path ");
    println!("--------------------------------|------------|-----------");
    for (host_path, file) in files {
        println!(" {:<30} |   {:>8x} | {}", file.name, file.size, host_path);
    }
    println!();
}
