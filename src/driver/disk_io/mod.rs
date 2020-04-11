use alloc::boxed::Box;
use alloc::vec::Vec;
use spin::Mutex;

mod ata_pio;
mod virtio;

pub trait BlockDevice: Send {
    /// Returns success status
    fn init(&mut self) -> bool;

    /// Sector size in bytes
    fn sector_size(&self) -> u64;

    /// Capacity in sectors
    fn capacity_sectors(&mut self) -> u64;

    /// Capacity in bytes
    fn capacity_bytes(&mut self) -> u64 {
        self.sector_size() * self.capacity_sectors()
    }

    fn read(&mut self, sector: u64) -> Vec<u8>;

    fn write(&mut self, sector: u64, data: Vec<u8>);
}

pub struct DiskController {
    pub driver: Option<Box<dyn BlockDevice>>,
}
impl DiskController {
    pub const fn new() -> DiskController {
        DiskController { driver: None }
    }

    pub unsafe fn init(&mut self) {
        log::debug!("DiskIO: Selecting driver...");

        // Initialize VirtIO controller if available
        self.driver = virtio::VirtioBlock::try_new();
        if self.driver.is_some() {
            log::info!("DiskIO: VirtIO-blk selected");
        } else {
            // Initialize ATA PIO controller if available
            self.driver = ata_pio::AtaPio::try_new();
            if self.driver.is_some() {
                log::info!("DiskIO: ATA PIO selected");
            } else {
                log::warn!("DiskIO: No supported devices found");
            }
        }

        if self.driver.is_some() {
            let ok = if let Some(ref mut driver) = self.driver {
                driver.init()
            } else {
                unreachable!()
            };

            if !ok {
                log::warn!("DiskIO: Driver initialization failed");
                self.driver = None;
            }
        }

        if let Some(ref mut driver) = self.driver {
            log::info!("DiskIO: Device capacity: {} bytes", driver.capacity_bytes());
        }
    }

    pub unsafe fn map<T>(
        &mut self, f: &mut dyn FnMut(&mut Box<dyn BlockDevice>) -> T,
    ) -> Option<T> {
        if let Some(ref mut driver) = self.driver {
            Some(f(driver))
        } else {
            None
        }
    }

    pub fn read(&mut self, sector: u64, count: u64) -> Vec<Vec<u8>> {
        if let Some(ref mut driver) = self.driver {
            log::info!("DiskIO: Read sectors {}..{}", sector, sector + count);
            (0..count)
                .map(|offset| driver.read(sector + offset))
                .collect()
        } else {
            panic!("DiskIO: No driver available");
        }
    }
}

// Create static pointer mutex with spinlock to make networking thread-safe
pub static DISK_IO: Mutex<DiskController> = Mutex::new(DiskController::new());

pub fn init() {
    let mut dc = DISK_IO.lock();
    unsafe {
        dc.init();
    }
}
