#![no_std]
#![feature(allocator_api)]
#![feature(no_more_cas)]
#![deny(unused_must_use)]

extern crate alloc;
extern crate libd7;

use alloc::vec::Vec;

use libd7::{ipc, select, syscall};

use fatfs::{Read, Write};

mod cache;
mod cursor;
mod disk;

use crate::cache::DiskAccess;
use crate::cursor::DiskCursor;
use crate::disk::Disk;

#[no_mangle]
fn main() -> ! {
    log::info!("daemon starting");

    // Storage backends
    // TODO: backend registers to us, instead of active waiting
    libd7::service::wait_for_one("driver_ata_pio");

    // Subscribe to client requests
    let server: ipc::Server<(), ()> = ipc::Server::exact("fatfs").unwrap();

    // Inform serviced that we are running.
    libd7::service::register("daemon_fatfs", false);

    log::info!("daemon running");

    let access = DiskAccess::new(
        Disk {
            sector_size: 0x200,
            read: |sector: u64| -> Vec<u8> {
                ipc::request("ata_pio/drive/1/read", (sector, 1u8)).expect("ata read")
            },
            write: |sector: u64, data: Vec<u8>| {
                ipc::deliver("ata_pio/drive/1/write", &(sector, data)).expect("ata write")
            },
        },
        2,
    );
    let c = DiskCursor::new(access);
    let fs = fatfs::FileSystem::new(c, fatfs::FsOptions::new()).expect("open fs");
    let mut cursor = fs.root_dir();

    let mut result = Vec::new();
    for entry in cursor.iter() {
        let entry = entry.expect("Entry");
        result.push(entry.file_name());
    }

    let mut cursor = fs.root_dir().open_dir("test/").expect("test/");

    let mut result = Vec::new();
    for entry in cursor.iter() {
        let entry = entry.expect("Entry");
        result.push(entry.file_name());
    }

    log::info!("{:?}", result);

    let mut file = fs.root_dir().create_file("example.txt").expect("open file");
    file.write(b"Example text\n").expect("Write failed");
    file.flush().expect("Close");

    todo!();
    // loop {
    //     select! {
    //         one(get_mac) => get_mac.handle(|()| Ok(device.mac_addr())).unwrap(),
    //     }
    // }
}
