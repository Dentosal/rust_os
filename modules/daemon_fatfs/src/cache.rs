use alloc::vec::Vec;
use lru::LruCache;

use crate::disk::Disk;

pub struct DiskAccess {
    cache: Option<LruCache<u64, Vec<u8>>>,
    disk: Disk,
}

impl DiskAccess {
    pub fn new(disk: Disk, cache_size: usize) -> Self {
        Self {
            disk,
            cache: if cache_size != 0 {
                Some(LruCache::new(cache_size))
            } else {
                None
            },
        }
    }

    pub fn sector_size(&self) -> usize {
        self.disk.sector_size
    }

    pub fn read(&mut self, sector: u64) -> Vec<u8> {
        if let Some(cache) = &mut self.cache {
            if let Some(data) = cache.get(&sector) {
                log::trace!("Cache hit");
                return data.clone();
            } else {
                log::trace!("Cache miss");
            }
        }

        let data = (self.disk.read)(sector);
        assert_eq!(data.len(), self.sector_size());
        if let Some(cache) = &mut self.cache {
            cache.put(sector, data.clone());
        }
        data
    }

    pub fn write(&mut self, sector: u64, data: Vec<u8>) {
        assert_eq!(data.len(), self.sector_size());
        if let Some(cache) = &mut self.cache {
            let _ = cache.put(sector, data.clone());
        }
        let data = (self.disk.write)(sector, data);
    }
}
