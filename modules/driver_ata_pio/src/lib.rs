#![no_std]
#![feature(asm)]
#![feature(alloc_prelude)]
#![feature(allocator_api)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

use alloc::prelude::v1::*;
use hashbrown::HashMap;
use libd7::{attachment::*, syscall};

mod ata_pio;

#[no_mangle]
fn main() -> ! {
    syscall::debug_print("ata pio driver starting");

    let controller = ata_pio::AtaPio::new();

    let attachments: Vec<_> = (0..controller.drive_count())
        .map(|drive_index| Leaf::new(&format!("/dev/ata_pio_{}", drive_index)).unwrap())
        .collect();

    let attachment_fds: Vec<_> = attachments.iter().map(|a| a.fd).collect();

    let mut cursors: HashMap<Sender, u64> = HashMap::new();

    loop {
        let avail_fd = syscall::fd_select(&attachment_fds, None).unwrap();
        let index = attachment_fds
            .iter()
            .position(|fd| *fd == avail_fd)
            .unwrap();

        let req = attachments[index].next_request().unwrap();

        let cur = *cursors.entry(req.sender).or_insert(0);

        match req.data {
            FileOperation::Read(byte_count) => {
                let sector_count = (byte_count / ata_pio::SECTOR_SIZE as u64) + 1;
                assert!(sector_count <= (core::u8::MAX as u64));
                let bytes = unsafe {
                    controller.read_lba(index, cur, sector_count as u8)
                };
                attachments[index].reply(req.response(bytes)).unwrap();
            }
            other => unimplemented!("Unsupported request: {:?}", other),
        }
    }
}
