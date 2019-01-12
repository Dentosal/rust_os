/// D7_StaticFS Driver

use d7staticfs::*;

use crate::disk_io::DISK_IO;

use alloc::prelude::*;

fn round_up_sector(p: u64) -> u64 {
    (p + SECTOR_SIZE - 1) / SECTOR_SIZE
}

/// Returns file list start sector
fn find_file_list() -> u32 {
    let mut dc = DISK_IO.lock();
    let mbr = dc.read(0, 1);
    u32::from_le_bytes([
        mbr[0][MBR_POSITION as usize + 0],
        mbr[0][MBR_POSITION as usize + 1],
        mbr[0][MBR_POSITION as usize + 2],
        mbr[0][MBR_POSITION as usize + 3],
    ])
}

/// Returns entry count
fn load_header(file_list_sector: u32) -> u32 {
    let mut dc = DISK_IO.lock();
    let header = &dc.read(file_list_sector as u64, 1)[0][..16];

    // check magic and version
    assert_eq!(header[0..4], HEADER_MAGIC.to_le_bytes(), "StaticFS: Incorrect magic header");
    assert_eq!(header[4..8], ONLY_VERSION.to_le_bytes(), "StaticFS: Incorrect version number");

    u32::from_le_bytes([header[8], header[9], header[10], header[11]])
}

/// Returns file entries and their sector positions
fn load_file_entries() -> Vec<(FileEntry, u32)> {
    let base_sector = find_file_list();
    let entry_count = load_header(base_sector);
    let sector_count = round_up_sector(16 + sizeof!(FileEntry) as u64 * entry_count as u64);
    let mut dc = DISK_IO.lock();
    let bytes: Vec<u8> = dc.read(base_sector as u64, sector_count).iter().flatten().skip(16).cloned().collect();

    let mut result = Vec::new();
    let mut position = base_sector + sector_count as u32;
    for chunk in bytes.chunks_exact(sizeof!(FileEntry)).take(entry_count as usize) {
        let file_entry = FileEntry::from_bytes([
            chunk[0x0], chunk[0x1], chunk[0x2], chunk[0x3],
            chunk[0x4], chunk[0x5], chunk[0x6], chunk[0x7],
            chunk[0x8], chunk[0x9], chunk[0xa], chunk[0xb],
            chunk[0xc], chunk[0xd], chunk[0xe], chunk[0xf],
        ]);
        if !file_entry.is_skip() {
            result.push((file_entry, position));
        }
        position += file_entry.size;
    }
    result
}

/// Returns (sector, size)
fn find_file(name: &str) -> Option<(u32, u32)> {
    for (file_entry, pos) in load_file_entries() {
        if file_entry.name_matches(name) {
            return Some((pos, file_entry.size));
        }
    }
    None
}

/// Returns (sector, size)
pub fn read_file(name: &str) -> Option<Vec<u8>> {
    let (p, s) = find_file(name)?;
    let mut dc = DISK_IO.lock();
    Some(dc.read(p as u64, s as u64).iter().flatten().cloned().collect())
}