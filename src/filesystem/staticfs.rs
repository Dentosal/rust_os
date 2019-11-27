//! `/dev/staticfs`
//! Static filesystem built into the kernel image
//! TODO: temporarily implemented with a disk driver

use d7staticfs::*;

use alloc::prelude::v1::*;
use hashbrown::HashMap;

use crate::driver::disk_io::DISK_IO;
use crate::filesystem::{error::*, FileClientId, FileOps, Leafness, Path, FILESYSTEM};

fn round_up_sector(p: u64) -> u64 {
    (p + SECTOR_SIZE - 1) / SECTOR_SIZE
}

pub struct StaticFSLeaf {
    /// Sector on disk
    start_sector: u32,
    /// Size on disk, in sectors
    size_sectors: u32,
    /// Data, loaded when requested for the first time
    data: Option<Vec<u8>>,
    /// Readers and offsets to the data
    readers: HashMap<FileClientId, usize>,
}
impl StaticFSLeaf {
    fn new(start_sector: u32, size_sectors: u32) -> Self {
        Self {
            start_sector,
            size_sectors,
            data: None,
            readers: HashMap::new(),
        }
    }

    fn ensure_loaded(&mut self) {
        if self.data.is_none() {
            let mut dc = DISK_IO.lock();
            self.data = Some(
                dc.read(self.start_sector as u64, self.size_sectors as u64)
                    .iter()
                    .flatten()
                    .cloned()
                    .collect(),
            );
        }
    }
}
impl FileOps for StaticFSLeaf {
    fn leafness(&self) -> Leafness {
        Leafness::Leaf
    }

    fn read(&mut self, fc: FileClientId, buf: &mut [u8]) -> IoResult<usize> {
        self.ensure_loaded();
        let data = self.data.as_ref().unwrap();
        let offset = self.readers.get(&fc).copied().unwrap_or(0);
        let count = (data.len() - offset).min(buf.len());
        buf[..count].copy_from_slice(&data[offset..offset + count]);
        self.readers.insert(fc, offset + count);
        Ok(count)
    }

    /// Remove reader when closing
    fn close(&mut self, fc: FileClientId) {
        self.readers.remove(&fc);
    }
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
    assert_eq!(
        header[0..4],
        HEADER_MAGIC.to_le_bytes(),
        "StaticFS: Incorrect magic header"
    );
    assert_eq!(
        header[4..8],
        ONLY_VERSION.to_le_bytes(),
        "StaticFS: Incorrect version number"
    );

    u32::from_le_bytes([header[8], header[9], header[10], header[11]])
}

/// Returns file entries and their sector positions
fn load_file_entries() -> Vec<(FileEntry, u32)> {
    let base_sector = find_file_list();
    if base_sector == 0xd7cafed7 {
        panic!("StaticFS missing");
    }
    let entry_count = load_header(base_sector);
    let sector_count = round_up_sector(16 + sizeof!(FileEntry) as u64 * entry_count as u64);
    let mut dc = DISK_IO.lock();
    let bytes: Vec<u8> = dc
        .read(base_sector as u64, sector_count)
        .iter()
        .flatten()
        .skip(16)
        .cloned()
        .collect();

    let mut result = Vec::new();
    let mut position = base_sector + sector_count as u32;
    for chunk in bytes
        .chunks_exact(sizeof!(FileEntry))
        .take(entry_count as usize)
    {
        let file_entry = FileEntry::from_bytes([
            chunk[0x0], chunk[0x1], chunk[0x2], chunk[0x3], chunk[0x4], chunk[0x5], chunk[0x6],
            chunk[0x7], chunk[0x8], chunk[0x9], chunk[0xa], chunk[0xb], chunk[0xc], chunk[0xd],
            chunk[0xe], chunk[0xf],
        ]);
        if !file_entry.is_skip() {
            result.push((file_entry, position));
        }
        position += file_entry.size;
    }
    result
}

pub fn init() {
    let mut fs = FILESYSTEM.lock();
    let _ = fs
        .create_static_branch(Path::new("/mnt/staticfs"))
        .expect("Could not create /dev/staticfs");

    for (file_entry, pos) in load_file_entries() {
        let mut name_bytes = file_entry.name.to_vec();
        while name_bytes.last() == Some(&0) {
            let _ = name_bytes.pop();
        }
        assert!(!name_bytes.is_empty());
        let name = String::from_utf8(name_bytes).expect("StaticFS: mon-UTF-8 filename");
        let leaf = StaticFSLeaf::new(pos, file_entry.size);
        fs.create_static(
            Path::new(&format!("/mnt/staticfs/{}", name)),
            Box::new(leaf),
        )
        .expect("Could not create file under /mnt/staticfs");
    }
}
