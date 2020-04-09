#![deny(unused_must_use)]

extern crate d7staticfs;

use std::env;
use std::fs::{File, OpenOptions};
use std::io::{prelude::*, SeekFrom};
use std::u64;

use d7staticfs::*;

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
            size_bytes: meta.len(),
        };
        files.push((rl_filename.to_owned(), fe));
    }

    let meta = {
        let f = File::open(disk_img_path).expect("Target file not found");
        f.metadata().expect("Target file metadata not available")
    };
    let size_sectors = meta.len() / SECTOR_SIZE;

    // Prepare header
    let header_body_contents: Vec<_> = files.iter().map(|(_, e)| e.clone()).collect();
    let header_body: Vec<u8> = pinecone::to_vec(&header_body_contents).unwrap();

    // Compute sizes
    let header_size_sectors = to_sectors_round_up(HEADER_SIZE_BYTES + header_body.len() as u64);
    let files_size_sectors: u64 = files.iter().map(|(_, e)| e.size_sectors()).sum();

    let required_size_sectors = kernel_skip_index as u64 + header_size_sectors + files_size_sectors;
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
        f.seek(SeekFrom::Start(MBR_POSITION as u64)).unwrap();
        f.read_exact(&mut magic_check).unwrap();
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
        f.read_exact(&mut empty_check).unwrap();
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

        let lba_ptr: [u8; 4] = (kernel_skip_index as u32).to_le_bytes();
        f.seek(SeekFrom::Start(MBR_POSITION as u64)).unwrap();
        f.write_all(&lba_ptr).unwrap();
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
        let header_2: [u8; 4] = (header_body.len() as u32).to_le_bytes();
        let header_3: [u8; 4] = 0u32.to_le_bytes();
        f.seek(SeekFrom::Start(kernel_skip_index as u64 * SECTOR_SIZE))
            .unwrap();

        // Header
        f.write_all(&header_0).unwrap();
        f.write_all(&header_1).unwrap();
        f.write_all(&header_2).unwrap();
        f.write_all(&header_3).unwrap();

        // Header body
        f.write_all(&header_body).unwrap();

        // Align to sector boundary
        let files_start_pos = kernel_skip_index as u64 + header_size_sectors;

        f.seek(SeekFrom::Start(files_start_pos * SECTOR_SIZE))
            .unwrap();
        let mut files_sectors_count: u64 = 0;

        // Copy files to disk image
        for (path, entry) in &files {
            let mut rf =
                File::open(path).unwrap_or_else(|_| panic!("Source file '{}' not found", path));

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
            let mut pad_count = SECTOR_SIZE - (counter % SECTOR_SIZE);
            if pad_count == SECTOR_SIZE {
                pad_count = 0;
            }
            f.write_all(&vec![0; pad_count as usize].as_slice())
                .unwrap();

            assert_eq!(to_sectors_round_up(counter), entry.size_sectors());
            files_sectors_count += entry.size_sectors();
        }

        assert_eq!(
            files_sectors_count,
            files.iter().map(|(_, e)| e.size_sectors()).sum::<u64>()
        );
    }

    println!(" File Name    | Size (hex) | Host Path ");
    println!("--------------|------------|-----------");
    for (host_path, file) in files {
        println!(
            " {:<12} | {:>8x} | {}",
            file.name, file.size_bytes, host_path
        );
    }
    println!();
}
