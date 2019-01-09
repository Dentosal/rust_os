// VirtIO block device driver
// https://wiki.osdev.org/Virtio

use crate::virtio;

use core::mem;
use core::ptr;
use alloc::boxed::Box;
use alloc::vec::Vec;

use mem_map::MEM_PAGE_SIZE_BYTES;

use super::BlockDevice;

const QUEUE_RX: u16 = 0;
const QUEUE_TX: u16 = 1;

bitflags! {
    struct Features: u32 {
        const REQUEST_BARRIERS    = 1 << 0;
        const SIZE_MAX            = 1 << 1;
        const SEG_MAX             = 1 << 2;
        const GEOMETRY            = 1 << 4;
        const READ_ONLY_LOCK      = 1 << 5;
        const BLOCK_SIZE          = 1 << 6;
        const SCSI_COMMANDS       = 1 << 7;
        const CACHE_FLUSH         = 1 << 9;
    }
}


pub struct BlockRequest {
    type_: u8, // 0: read, 1: write
    unused: u8, // unused in 1.0 spec
    sector: u64,
}
impl BlockRequest {
    pub fn new_read(sector: u64) -> BlockRequest {
        BlockRequest {
            type_: 0,
            unused: 0,
            sector
        }
    }
    pub fn new_write(sector: u64) -> BlockRequest {
        BlockRequest {
            type_: 1,
            unused: 0,
            sector
        }
    }
}

pub struct VirtioBlock {
    device: virtio::VirtioDevice,
    queues: Vec<virtio::VirtQueue>,
}
impl VirtioBlock {
    pub fn try_new() -> Option<Box<BlockDevice>> {
        if let Some(mut device) = virtio::VirtioDevice::try_new(virtio::DeviceType::BlockDevice) {
            // Update device state
            device.write::<u8>(0x12, virtio::DeviceStatus::ACKNOWLEDGE.bits());

            Some(box VirtioBlock {
                device,
                queues: Vec::new(),
            })
        }
        else {
            None
        }
    }
}
impl BlockDevice for VirtioBlock {
    fn init(&mut self) -> bool {
        // Tell the device that it's supported by this driver
        self.device.write::<u8>(0x12, virtio::DeviceStatus::STATE_LOADED.bits());


        // Feature negotiation
        let mut feature_bits = self.device.features();
        // feature_bits &= !Features::READ_ONLY_LOCK; // Turn off read-only safety lock
        feature_bits &= !Features::BLOCK_SIZE.bits(); // Disable block size detection

        if !self.device.set_features(feature_bits) {
            return false;
        }

        // Memory queues
        self.queues = self.device.init_queues();

        // Update device state
        self.device.write::<u8>(0x12, virtio::DeviceStatus::STATE_READY.bits());

        rprintln!("VirtIO-blk: Device ready");

        // Success
        true
    }

    fn capacity_bytes(&mut self) -> u64 {
        let lo = self.device.read::<u32>(0x14);
        let hi = self.device.read::<u32>(0x18);
        (((hi as u64) << 32u64) | (lo as u64)) * 0x200
    }

    fn read(&mut self, sector: u64) -> Vec<u8> {
        Vec::new()
    }

    fn write(&mut self, sector: u64, data: Vec<u8>) {
        // assert!(packet.len() <= 0xffff);


        // self.device.select_queue(QUEUE_TX);
        // let mut tx_queue = &mut self.queues[QUEUE_TX as usize];

        // use crate::HEAP_ALLOCATOR;
        // use core::alloc::{GlobalAlloc, Layout};

        // let length = packet.len() + mem::size_of::<NetHeader>();
        // let layout = Layout::from_size_align(
        //     length,
        //     MEM_PAGE_SIZE_BYTES // page aligned
        // ).unwrap();

        // let buffer: *mut u8 = unsafe { HEAP_ALLOCATOR.alloc(layout) } as *mut u8;

        // let header = NetHeader::new(packet.len() as u16);

        // unsafe {
        //     ptr::write_volatile(buffer as *mut NetHeader, header);
        //     for i in 0..packet.len() {
        //         ptr::write_volatile(
        //             buffer.offset(mem::size_of::<NetHeader>() as isize + i as isize),
        //             packet[i]
        //         );
        //     }
        // }


        // unsafe {
        //     for i in 0..(mem::size_of::<NetHeader>()) {
        //         rprint!("{:02x} ", ptr::read_volatile(
        //             buffer.offset(i as isize)
        //         ));
        //     }
        // rprintln!("\nHeader over");
        //     for i in 0..packet.len() {
        //         rprint!("{:02x} ", ptr::read_volatile(
        //             buffer.offset(mem::size_of::<NetHeader>() as isize + i as isize)
        //         ));
        //     }
        // }
        // rprintln!("\nPacket over");

        // unsafe {
        //     assert!(ptr::read_volatile(buffer) == 0);
        //     assert!(ptr::read_volatile(buffer.offset(mem::size_of::<NetHeader>() as isize)) == packet[0]);
        //     assert!(ptr::read_volatile(buffer.offset((mem::size_of::<NetHeader>() + packet.len() - 1) as isize)) == packet[packet.len()-1]);
        // }


        // let buf_index = tx_queue.find_free().expect("No tx queue slots free");

        // // Set buffer descriptor
        // let desc = virtio::VirtQueueDesc::new_read(buffer as u64, length as u32);
        // tx_queue.set_desc_at(buf_index, desc);

        // // Add it in the available ring
        // let mut ah = tx_queue.available_header();
        // let index = ah.index % tx_queue.queue_size;
        // tx_queue.set_available_ring(index, buf_index);
        // ah.index += 1;
        // tx_queue.set_available_header(ah);

        // use time::sleep_ms;
        // sleep_ms(10);


        // // Notify the device about the change
        // self.device.queue_notify(QUEUE_TX);
        // rprintln!("VirtIO-blk: TX Queue notify");

        // unsafe { HEAP_ALLOCATOR.dealloc(buffer as *mut u8, layout) };
    }

}
