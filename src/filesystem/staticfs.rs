//! `/dev/staticfs`
//! Static filesystem built into the kernel image
//! TODO: temporarily implemented with a disk driver

use d7staticfs::*;

use alloc::prelude::v1::*;
use hashbrown::HashMap;

use crate::driver::disk_io::DISK_IO;
use crate::filesystem::{error::*, CloseAction, FileClientId, FileOps, Leafness, Path, FILESYSTEM};
use crate::multitasking::WaitFor;

pub struct StaticFSLeaf {
    /// Sector on disk
    start_sector: u32,
    /// Size on disk, in bytes
    size_bytes: u64,
    /// Data, loaded when requested for the first time
    data: Option<Vec<u8>>,
    /// Readers and offsets to the data
    readers: HashMap<FileClientId, usize>,
}
impl StaticFSLeaf {
    fn new(start_sector: u32, size_bytes: u64) -> Self {
        Self {
            start_sector,
            size_bytes,
            data: None,
            readers: HashMap::new(),
        }
    }

    fn ensure_loaded(&mut self) {
        if self.data.is_none() {
            let mut dc = DISK_IO.lock();
            self.data = Some(
                dc.read(
                    self.start_sector as u64,
                    to_sectors_round_up(self.size_bytes),
                )
                .iter()
                .flatten()
                .take(self.size_bytes as usize)
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
        IoResult::Success(count)
    }

    fn read_waiting_for(&mut self, fc: FileClientId) -> WaitFor {
        WaitFor::None
    }

    /// Remove reader when closing
    fn close(&mut self, fc: FileClientId) -> IoResult<CloseAction> {
        self.readers.remove(&fc);
        IoResult::Success(CloseAction::Normal)
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

/// Returns header body size, in bytes
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
    let header_body_size = load_header(base_sector) as u64;
    let sector_count = to_sectors_round_up(HEADER_SIZE_BYTES + header_body_size);
    let mut dc = DISK_IO.lock();
    let bytes: Vec<u8> = dc
        .read(base_sector as u64, sector_count)
        .iter()
        .flatten()
        .skip(HEADER_SIZE_BYTES as usize)
        .take(header_body_size as usize)
        .cloned()
        .collect();

    let file_list: Vec<FileEntry> =
        pinecone::from_bytes(&bytes[..]).expect("Could not deserialize staticfs file list");

    let mut result = Vec::new();
    let mut current_sector_offset = 0;
    for file in file_list {
        let size_sectors = file.size_sectors() as u32;
        result.push((
            file,
            base_sector + sector_count as u32 + current_sector_offset,
        ));
        current_sector_offset += size_sectors;
    }
    result
}

pub fn init() {
    let mut fs = FILESYSTEM.lock();
    let _ = fs
        .create_static_branch(Path::new("/mnt/staticfs"))
        .expect("Could not create /dev/staticfs");

    for (file_entry, pos) in load_file_entries() {
        assert!(!file_entry.name.is_empty());
        let leaf = StaticFSLeaf::new(pos, file_entry.size_bytes);
        fs.create_static(
            Path::new(&format!("/mnt/staticfs/{}", file_entry.name)),
            Box::new(leaf),
        )
        .expect("Could not create file under /mnt/staticfs");
    }
}
