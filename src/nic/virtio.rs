// VirtIO network driver
// https://wiki.osdev.org/Virtio

use core::mem;
use core::ptr;
use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::string::String;

use mem_map::MEM_PAGE_SIZE_BYTES;

use super::NIC;
use virtio;

const QUEUE_RX: u16 = 0;
const QUEUE_TX: u16 = 1;

/// https://wiki.osdev.org/Virtio#Network_Packets
pub struct NetHeader {
    flags: u8,
    segmentation_type: u8,
    header_length: u16,
    segment_max_size: u16,
    checksum_start: u16,
    checksum_offset: u16,
    buffer_count: u16,
}
impl NetHeader {
    pub fn new(size: u16) -> NetHeader {
        NetHeader {
            flags: 0, // automatically add checksum
            // flags: 1, // automatically add checksum
            segmentation_type: 0, // none
            header_length: 0,
            segment_max_size: 0,
            checksum_start: 0,
            checksum_offset: size,
            buffer_count: 0, // unused on transmissed packages
        }
    }
}

pub struct VirtioNet {
    device: virtio::VirtioDevice,
    queues: Vec<virtio::VirtQueue>,
    mac_address: [u8; 6],
}
impl VirtioNet {
    pub fn try_new() -> Option<Box<NIC>> {
        if let Some(mut device) = virtio::VirtioDevice::try_new(virtio::DeviceType::NetworkCard) {
            // Update device state
            device.write::<u8>(0x12, virtio::DeviceStatus::ACKNOWLEDGE.bits());

            Some(box VirtioNet {
                device,
                queues: Vec::new(),
                mac_address: [0; 6],
            })
        }
        else {
            None
        }
    }
}
impl NIC for VirtioNet {
    fn init(&mut self) -> bool {
        // Tell the device that it's supported by this driver
        self.device.write::<u8>(0x12, virtio::DeviceStatus::STATE_LOADED.bits());

        // Read MAC address
        for i in 0..6u16 {
            self.mac_address[i as usize] = self.device.read::<u8>(0x14 + i);
        }

        // Feature negotiation
        let mut feature_bits = self.device.features();
        feature_bits |= !(1 <<  5); // Require MAC address
        feature_bits |= !(1 << 17); // Require status field
        feature_bits &= !(1 << 17); // Disable control queue
        feature_bits |= !(1 <<  0); // Enable checksums
        // Disable (sometimes buggy) tcp/udp packet size
        feature_bits &= !(1 <<  7);
        feature_bits &= !(1 <<  8);
        feature_bits &= !(1 << 10);
        feature_bits &= !(1 << 15);
        feature_bits &= !(1 << 29);

        if !self.device.set_features(feature_bits) {
            return false;
        }

        // Memory queues
        self.queues = self.device.init_queues();

        // Update device state
        self.device.write::<u8>(0x12, virtio::DeviceStatus::STATE_READY.bits());

        rprintln!("VirtIO-net: Device ready");

        // Success
        true
    }

    fn send(&mut self, packet: Vec<u8>) {
        assert!(packet.len() <= 0xffff);


        self.device.select_queue(QUEUE_TX);
        let mut tx_queue = &mut self.queues[QUEUE_TX as usize];

        use crate::HEAP_ALLOCATOR;
        use core::alloc::{GlobalAlloc, Layout};

        let length = packet.len() + mem::size_of::<NetHeader>();
        let layout = Layout::from_size_align(
            length,
            MEM_PAGE_SIZE_BYTES // page aligned
        ).unwrap();

        let buffer: *mut u8 = unsafe { HEAP_ALLOCATOR.alloc(layout) } as *mut u8;

        let header = NetHeader::new(packet.len() as u16);

        unsafe {
            ptr::write_volatile(buffer as *mut NetHeader, header);
            for i in 0..packet.len() {
                ptr::write_volatile(
                    buffer.offset(mem::size_of::<NetHeader>() as isize + i as isize),
                    packet[i]
                );
            }
        }


        unsafe {
            for i in 0..(mem::size_of::<NetHeader>()) {
                rprint!("{:02x} ", ptr::read_volatile(
                    buffer.offset(i as isize)
                ));
            }
        rprintln!("\nHeader over");
            for i in 0..packet.len() {
                rprint!("{:02x} ", ptr::read_volatile(
                    buffer.offset(mem::size_of::<NetHeader>() as isize + i as isize)
                ));
            }
        }
        rprintln!("\nPacket over");

        unsafe {
            assert!(ptr::read_volatile(buffer) == 0);
            assert!(ptr::read_volatile(buffer.offset(mem::size_of::<NetHeader>() as isize)) == packet[0]);
            assert!(ptr::read_volatile(buffer.offset((mem::size_of::<NetHeader>() + packet.len() - 1) as isize)) == packet[packet.len()-1]);
        }


        let buf_index = tx_queue.find_free().expect("No tx queue slots free");

        // Set buffer descriptor
        let desc = virtio::VirtQueueDesc::new_read(buffer as u64, length as u32);
        tx_queue.set_desc_at(buf_index, desc);

        // Add it in the available ring
        let mut ah = tx_queue.available_header();
        let index = ah.index % tx_queue.queue_size;
        tx_queue.set_available_ring(index, buf_index);
        ah.index += 1;
        tx_queue.set_available_header(ah);

        use time::sleep_ms;
        sleep_ms(10);


        // Notify the device about the change
        self.device.queue_notify(QUEUE_TX);
        rprintln!("VirtIO-net: TX Queue notify");

        unsafe { HEAP_ALLOCATOR.dealloc(buffer as *mut u8, layout) };
    }

    fn mac_addr(&self) -> [u8; 6] {
        self.mac_address
    }
}
