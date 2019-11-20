// VirtIO block device driver
// https://wiki.osdev.org/Virtio

use alloc::prelude::v1::*;
use core::mem;
use core::ptr;
use volatile;

use crate::driver::virtio;

use super::BlockDevice;

bitflags! {
    /// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-2050003
    struct Features: u32 {
        const LEGACY_BARRIER      = 1 << 0;
        const SIZE_MAX            = 1 << 1;
        const SEG_MAX             = 1 << 2;
        const GEOMETRY            = 1 << 4;
        const READ_ONLY           = 1 << 5;
        const BLOCK_SIZE          = 1 << 6;
        const LEAGCY_SCSI         = 1 << 9;
        const CACHE_FLUSH         = 1 << 9;
        const TOPOLOGY            = 1 << 10;
        const CONFIG_WCE          = 1 << 11;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
struct BlockRequest {
    type_: u32,  // 0: read, 1: write, 5: flush
    ioprio: u32, // IO priority, unused in 1.0 spec
    sector: u64, // sector number
    data: [u8; 0x200],
    status: u8, // 0: ok, 1: error, 2: unsupported
}
impl BlockRequest {
    pub const HEADER_SIZE: usize = 16;
    pub const CONTENT_SIZE: usize = 0x200;
    pub const FOOTER_SIZE: usize = 1;

    pub fn new_read(sector: u64) -> BlockRequest {
        BlockRequest {
            type_: 0,
            ioprio: 0,
            sector,
            data: [0; 0x200],
            status: 0xff,
        }
    }
    pub fn new_write(sector: u64) -> BlockRequest {
        BlockRequest {
            type_: 1,
            ..Self::new_read(sector)
        }
    }
    pub fn new_flush() -> BlockRequest {
        BlockRequest {
            type_: 4,
            ..Self::new_read(0)
        }
    }
}

pub struct VirtioBlock {
    device: virtio::VirtioDevice,
    queues: Vec<virtio::VirtQueue>,
}
impl VirtioBlock {
    pub fn try_new() -> Option<Box<dyn BlockDevice>> {
        if let Some(mut device) = virtio::VirtioDevice::try_new(virtio::DeviceType::BlockDevice) {
            // Update device state
            device.set_status(virtio::DeviceStatus::ACKNOWLEDGE);

            Some(box VirtioBlock {
                device,
                queues: Vec::new(),
            })
        } else {
            None
        }
    }
}
impl BlockDevice for VirtioBlock {
    fn init(&mut self) -> bool {
        // Tell the device that it's supported by this driver
        self.device.set_status(virtio::DeviceStatus::STATE_LOADED);

        // Feature negotiation
        let mut feature_bits = self.device.features();
        // TODO: Accept Read-Only device, and mark this as read-only
        // feature_bits ??? Features::READ_ONLY;

        if !self.device.set_features(feature_bits) {
            return false;
        }

        // Memory queues
        self.queues = self.device.init_queues();
        assert_eq!(self.queues.len(), 1);

        // Update device state
        self.device.set_status(virtio::DeviceStatus::STATE_READY);

        rprintln!("VirtIO-blk: Device ready");

        // Success
        true
    }

    fn sector_size(&self) -> u64 {
        0x200
    }

    fn capacity_sectors(&mut self) -> u64 {
        self.device.read_dev_config::<u64>(0x00)
    }

    fn read(&mut self, sector: u64) -> Vec<u8> {
        let req = volatile::ReadOnly::new(BlockRequest::new_read(sector));

        let mut queue = &mut self.queues[0];
        let buf_indices = queue
            .find_free_n(2)
            .expect("VirtIO-blk: Not enough queue slots free");

        // Set buffer descriptor
        let mut desc0 = virtio::VirtQueueDesc::new_read(
            (&req) as *const _ as u64,
            BlockRequest::HEADER_SIZE as u32,
        );
        let desc1 = virtio::VirtQueueDesc::new_write(
            (&req) as *const _ as u64 + BlockRequest::HEADER_SIZE as u64,
            BlockRequest::CONTENT_SIZE as u32 + BlockRequest::FOOTER_SIZE as u32,
        );

        desc0.chain(buf_indices[1]);

        queue.set_desc_at(buf_indices[0], desc0);
        queue.set_desc_at(buf_indices[1], desc1);

        // Add it in the available ring
        let mut ah = queue.available_header();
        let index = ah.index % queue.queue_size;
        queue.set_available_item(index, buf_indices[0]);
        ah.index = ah.index.wrapping_add(1);
        queue.set_available_header(ah);

        // Notify the device about the change
        self.device.notify(0);

        // Poll status byte
        let data = loop {
            let req_done = req.read();
            match req_done.status {
                0xff => {},
                0 => break req_done.data,
                1 => panic!("VirtIO read failed (1 - IOERR)"),
                2 => panic!("VirtIO read failed (2 - UNSUPP)"),
                n => panic!("VirtIO read failed ({} - ?)", n),
            }

            use crate::time::sleep_ms;
            sleep_ms(1);
        };

        data.to_vec()
    }

    fn write(&mut self, sector: u64, data: Vec<u8>) {}
}
