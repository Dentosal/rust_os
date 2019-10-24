pub mod staticfs;

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use d7ramfs;

// TODO: FileSystem trait
type FileSystem = d7ramfs::RamFS;

struct VirtualFS {
    mounted: Vec<(String, FileSystem)>,
}
impl VirtualFS {
    pub const fn new() -> VirtualFS {
        VirtualFS {
            mounted: Vec::new(),
        }
    }

    fn create_ramfs(&mut self, name: &str) {
        self.mounted.push((name.to_owned(), d7ramfs::RamFS::new()));
    }
}

static FILESYSTEM: Mutex<VirtualFS> = Mutex::new(VirtualFS::new());

pub fn init() {
    FILESYSTEM.lock().create_ramfs("RamFS");
}
