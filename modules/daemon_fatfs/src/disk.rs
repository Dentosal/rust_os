use alloc::vec::Vec;

pub struct Disk {
    pub sector_size: usize,
    pub read: fn(u64) -> Vec<u8>,
    pub write: fn(u64, Vec<u8>),
}
