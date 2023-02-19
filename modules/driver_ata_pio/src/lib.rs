#![no_std]
#![feature(allocator_api)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

extern crate libd7;

use alloc::vec::Vec;
use libd7::ipc::InternalSubscription;
use libd7::{ipc, select};

mod ata_pio;

#[no_mangle]
fn main() -> ! {
    log::info!("driver starting");

    let controller = ata_pio::AtaPio::new();

    let drive_count = controller.drive_count();
    assert!(drive_count > 0, "No drives found");

    let drive_info: Vec<u64> = (0..drive_count)
        .map(|i| controller.capacity_sectors(i))
        .collect();

    log::info!("drives found {:?}", drive_info);

    let info: ipc::Server<(), Vec<u64>> = ipc::Server::exact("ata_pio/drives").unwrap();
    let drive_read: Vec<ipc::Server<(u64, u8), Vec<u8>>> = (0..drive_count)
        .map(|i| ipc::Server::exact(&format!("ata_pio/drive/{}/read", i)).unwrap())
        .collect();
    let drive_write: Vec<ipc::ReliableSubscription<(u64, Vec<u8>)>> = (0..drive_count)
        .map(|i| ipc::ReliableSubscription::exact(&format!("ata_pio/drive/{}/write", i)).unwrap())
        .collect();

    let read_sub_ids: Vec<_> = drive_read.iter().map(|s| s.sub_id()).collect();
    let write_sub_ids: Vec<_> = drive_write.iter().map(|s| s.sub_id()).collect();

    // Inform serviced that we are running.
    libd7::service::register("driver_ata_pio", false);

    loop {
        select! {
            any(read_sub_ids) -> i => {
                drive_read[i].handle(|(sector, count)| {
                    // TODO: check that the sector index is valid
                    Ok(unsafe {controller.read_lba(i, sector, count)})
                }).unwrap();
            },
            any(write_sub_ids) -> i => {
                let (ack_ctx, (sector, data)) = drive_write[i].receive().unwrap();
                // TODO: check that the sector index is valid
                unsafe {controller.write_lba(i, sector, &data)};
                ack_ctx.ack().unwrap();
            },
            one(info) => {
                info.handle(|()| Ok(drive_info.clone())).unwrap();
            }
        };
    }
}
