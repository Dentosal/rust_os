// Generic VirtIO driver
// https://wiki.osdev.org/Virtio
// https://ozlabs.org/~rusty/virtio-spec/virtio-0.9.5.pdf
// http://docs.oasis-open.org/virtio/virtio/v1.0/cs04/virtio-v1.0-cs04.html#x1-300005
// http://www.dumais.io/index.php?article=aca38a9a2b065b24dfa1dee728062a12

use core::mem;
use core::ptr;
use cpuio::UnsafePort;
use alloc::vec::Vec;

use pci;
use mem_map::MEM_PAGE_SIZE_BYTES;
use memory::dma_allocator::DMA_ALLOCATOR;

/// Align upwards to page size
pub fn page_align_up(addr: usize) -> usize {
    (addr + MEM_PAGE_SIZE_BYTES - 1) & !(MEM_PAGE_SIZE_BYTES - 1)
}


bitflags! {
    pub struct BufferFlags: u16 {
        /// The buffer continues in the next field
        const NEXT = 0b00000001;
        /// Buffer is write-only (clear for read-only)
        const WRITE = 0b00000010;
        /// Buffer contains additional buffer addresses
        const INDIRECT = 0b00000100;
    }
}

#[repr(C)]
pub struct VirtQueueDesc {
    /// Address of the buffer on the guest machine (physical address)
    address: u64,
    /// Length of the buffer
    length: u32,
    /// Flags
    flags: BufferFlags,
    /// If flag is set, contains index of next buffer in chain
    next: u16,
}
impl VirtQueueDesc {
    pub fn new_read(address: u64, length: u32) -> VirtQueueDesc {
        VirtQueueDesc {
            address,
            length,
            flags: BufferFlags::empty(),
            next: 0,
        }
    }
}


#[repr(C)]
pub struct AvailableHeader {
    pub flags: u16,
    pub index: u16,
}

#[repr(C)]
struct UsedItem {
    index: u32,
    length: u32,
}

// #[repr(C, align(0x1000))]
// struct Used {
//     flags: u16,
//     index: u16,
//     rings[]: used_item,
// }

pub struct VirtQueue {
    pointer: *mut u8,
    pub queue_size: u16,
}
unsafe impl Send for VirtQueue {}
impl VirtQueue {
    fn calc_sizes(queue_size: u16) -> (usize, usize, usize) {
        let size_buffers = (queue_size as usize) * mem::size_of::<VirtQueueDesc>();
        let size_available = (3 + (queue_size as usize)) * mem::size_of::<u16>();


        let size_used = 3 * mem::size_of::<u16>() + (queue_size as usize) * mem::size_of::<UsedItem>();

        (
            size_buffers,
            size_available,
            size_used,
        )
    }

    fn calc_size(queue_size: u16) -> usize {
        let (a, b, c) = VirtQueue::calc_sizes(queue_size);
        page_align_up(a + b) + page_align_up(c)
    }

    pub fn new(queue_size: u16) -> VirtQueue {
        let vq_size_bytes = VirtQueue::calc_size(queue_size);
        assert!(vq_size_bytes % MEM_PAGE_SIZE_BYTES == 0);

        let pointer = {
            DMA_ALLOCATOR
                .lock()
                .allocate_blocks(vq_size_bytes / MEM_PAGE_SIZE_BYTES)
                .expect("Could not allocate DMA buffer for VirtIO")
        };

        VirtQueue {
            pointer,
            queue_size,
        }
    }

    pub fn addr(&self) -> usize {
        self.pointer as usize
    }

    unsafe fn zero(&mut self) {
        for i in 0..VirtQueue::calc_size(self.queue_size) {
            ptr::write_volatile(self.pointer.offset(i as isize), 0);
        }
    }

    pub fn create(&mut self) {
        // Zeroing means:
        // * empty descriptors
        // * empty available
        // * empty used
        unsafe {
            self.zero();
        }
    }

    pub fn find_free(&self) -> Option<u16> {
        for index in 0..self.queue_size {
            if self.desc_at(index).length == 0 {
                return Some(index);
            }
        }
        None
    }

    pub fn desc_at(&self, index: u16) -> VirtQueueDesc {
        assert!(index < self.queue_size);
        unsafe {
            let p = self.pointer.offset((index * (mem::size_of::<VirtQueueDesc>() as u16)) as isize);
            ptr::read_volatile(p as *const VirtQueueDesc)
        }
    }

    pub fn set_desc_at(&self, index: u16, desc: VirtQueueDesc) {
        assert!(index < self.queue_size);
        unsafe {
            let p = self.pointer.offset((index * (mem::size_of::<VirtQueueDesc>() as u16)) as isize);
            ptr::write_volatile(p as *mut VirtQueueDesc, desc);
        }
    }

    pub fn available_header(&self) -> AvailableHeader {
        unsafe {
            let p = self.pointer.offset(VirtQueue::calc_sizes(self.queue_size).0 as isize);
            ptr::read_volatile(p as *const AvailableHeader)
        }
    }

    pub fn set_available_header(&self, header: AvailableHeader) {
        unsafe {
            let p = self.pointer.offset(VirtQueue::calc_sizes(self.queue_size).0 as isize);
            ptr::write_volatile(p as *mut AvailableHeader, header);
        }
    }

    pub fn set_available_ring(&self, index: u16, value: u16) {
        assert!(index < self.queue_size);

        unsafe {
            let p = self.pointer.offset((
                VirtQueue::calc_sizes(self.queue_size).0 +
                mem::size_of::<AvailableHeader>() +
                (index as usize) * mem::size_of::<u16>()
            ) as isize);
            ptr::write_volatile(p as *mut u16, value);
        }
    }
}

bitflags! {
    pub struct DeviceStatus: u8 {
        const ACKNOWLEDGE   = 0b00000001;
        const DRIVER        = 0b00000010;
        const DRIVER_OK     = 0b00000100;
        const FEATURES_OK   = 0b00001000;
        const DEVICE_ERROR  = 0b01000000;
        const DRIVER_ERROR  = 0b10000000;

        const STATE_LOADED = Self::ACKNOWLEDGE.bits | Self::DRIVER.bits;
        const STATE_READY  = Self::STATE_LOADED.bits | Self::DRIVER_OK.bits | Self::FEATURES_OK.bits;
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum DeviceType {
    NetworkCard = 1,
    BlockDevice = 2,
    Console = 3,
    EntropySource = 4,
    MemoryBallooning = 5,
    IOMemory = 6,
    RPMSG = 7,
    SCSIHost = 8,
    Transport_9P = 9,
    MAC_802_11_WLAN = 10,
}

pub struct VirtioDevice {
    io_base: u16,
}
impl VirtioDevice {
    fn find_pci_device(device_type: DeviceType) -> Option<pci::Device> {
        let mut pci_ctrl = pci::PCI.lock();
        pci_ctrl.find(|dev| {
            //	VirtIO
            dev.vendor == 0x1af4 && (0x1000 <= dev.id && dev.id <= 0x103f)
            // Requested device type
            && dev.subsystem_id() == device_type as u16
        })
    }

    pub fn try_new(device_type: DeviceType) -> Option<VirtioDevice> {
        let pci_device = VirtioDevice::find_pci_device(device_type)?;

        let bars = unsafe {pci_device.get_bars()};
        let io_base = (bars[0] & !0x3) as u16;

        Some(VirtioDevice {
            io_base,
        })
    }


    pub fn read<T: cpuio::InOut>(&mut self, offset: u16) -> T {
        unsafe {
            UnsafePort::<T>::new(self.io_base + offset).read()
        }
    }

    pub fn write<T: cpuio::InOut>(&mut self, offset: u16, value: T) {
        unsafe {
            UnsafePort::<T>::new(self.io_base + offset).write(value)
        }
    }

    pub fn select_queue(&mut self, index: u16) {
        self.write::<u16>(0x0e, index);
    }

    /// Feature bits
    pub fn features(&mut self) -> u32 {
        self.read::<u32>(0x00)
    }

    /// Set feature bits
    /// # Returns
    /// Was setting features successful
    pub fn set_features(&mut self, bits: u32) -> bool {
        self.write::<u32>(0x04, bits);

        // Test if the FEATURES_OK bit can be set
        self.write::<u8>(
            0x12,
            DeviceStatus::STATE_READY.bits() | DeviceStatus::FEATURES_OK.bits()
        );

        if (self.read::<u8>(0x12) & DeviceStatus::FEATURES_OK.bits()) == 0 {
            rprintln!("VirtIO feature negotiation failed.");
            return false;
        }
        true
    }

    /// Queue size (element count) for selected queue
    pub fn queue_size(&mut self) -> u16 {
        self.read::<u16>(0x0c)
    }

    /// Queue size (element count) for selected queue
    fn set_queue_addr(&mut self, addr: usize) {
        let p = addr >> 12; // divide by 0x1000 (page size)
        assert!(p <= 0xffff_ffff);
        self.write::<u32>(0x08, p as u32)
    }

    /// Notify about a change in a queue
    pub fn queue_notify(&mut self, index: u16) {
        self.write::<u16>(0x10, index);
    }

    fn init_current_queue(&mut self) -> VirtQueue {
        let queue_size = self.queue_size();

        let mut vq = VirtQueue::new(queue_size);
        vq.create();
        self.set_queue_addr(vq.addr());
        vq
    }

    pub fn init_queues(&mut self) -> Vec<VirtQueue> {
        let mut result = Vec::new();
        for index in 0..16u16 {
            self.select_queue(index);
            if self.queue_size() == 0 {
                break;
            }
            result.push(self.init_current_queue());
        }
        result
    }
}
