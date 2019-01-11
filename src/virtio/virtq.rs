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
        /// Buffer is write-only FOR THE DEVICE (clear for read-only)
        const WRITE = 0b00000010;
        /// Buffer contains additional buffer addresses
        const INDIRECT = 0b00000100;
    }
}

#[derive(Debug, Copy, Clone)]
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

    pub fn new_write(address: u64, length: u32) -> VirtQueueDesc {
        VirtQueueDesc {
            flags: BufferFlags::WRITE,
            ..Self::new_read(address, length)
        }
    }

    pub fn chain(&mut self, next_index: u16) {
        self.flags |= BufferFlags::NEXT;
        self.next = next_index;
    }
}


#[repr(C)]
pub struct AvailableHeader {
    pub flags: u16,
    pub index: u16,
}

#[repr(C)]
pub struct AvailableItem {
    pub desc_index: u16,
}

#[repr(C)]
pub struct AvailableFooter {
    pub used_event: u16,
}

#[repr(C)]
pub struct UsedHeader {
    flags: u32,
    index: u32,
}

#[repr(C)]
pub struct UsedItem {
    index: u32,
    length: u32,
}

#[repr(C)]
pub struct UsedFooter {
    pub available_event: u16,
}

pub struct VirtQueue {
    pointer: *mut u8,
    pub queue_size: u16,
}
unsafe impl Send for VirtQueue {}
impl VirtQueue {
    fn calc_sizes(queue_size: usize) -> (usize, usize, usize) {
        let size_descriptors =     queue_size * mem::size_of::<VirtQueueDesc>();
        let size_available   = 6 + queue_size * mem::size_of::<u16>();
        let size_used        = 6 + queue_size * mem::size_of::<UsedItem>();

        (
            size_descriptors,
            size_available,
            size_used,
        )
    }

    fn calc_size(queue_size: usize) -> usize {
        let (a, b, c) = VirtQueue::calc_sizes(queue_size);
        page_align_up(a + b + c)
    }

    pub fn new(queue_size: u16) -> VirtQueue {
        let vq_size_bytes = VirtQueue::calc_size(queue_size as usize);
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

    pub fn sizes(&self) -> (usize, usize, usize) {
        Self::calc_sizes(self.queue_size as usize)
    }

    pub fn zero(&mut self) {
        // Zeroing means:
        // * empty descriptors
        // * empty available
        // * empty used
        for i in 0..VirtQueue::calc_size(self.queue_size as usize) {
            unsafe {
                ptr::write_volatile(self.pointer.offset(i as isize), 0u8);
            }
        }
    }

    /// Find a free descriptor
    pub fn find_free(&self) -> Option<u16> {
        for index in 0..self.queue_size {
            if self.desc_at(index).length == 0 {
                return Some(index);
            }
        }
        None
    }

    /// Find n free descriptors. None if all not found.
    pub fn find_free_n(&self, n: usize) -> Option<Vec<u16>> {
        assert!(n > 0);

        let mut result = Vec::with_capacity(n);
        for index in 0..self.queue_size {
            if self.desc_at(index).length == 0 {
                result.push(index);
                if result.len() == n {
                    return Some(result);
                }
            }
        }
        None
    }

    /// Get descriptor at index
    pub fn desc_at(&self, index: u16) -> VirtQueueDesc {
        assert!(index < self.queue_size);
        unsafe {
            let p = self.pointer.offset((index * (mem::size_of::<VirtQueueDesc>() as u16)) as isize);
            ptr::read_volatile(p as *const VirtQueueDesc)
        }
    }

    /// Set descriptor at index
    pub fn set_desc_at(&self, index: u16, desc: VirtQueueDesc) {
        assert!(index < self.queue_size);

        unsafe {
            let p = self.pointer.offset((index * (mem::size_of::<VirtQueueDesc>() as u16)) as isize);
            ptr::write_volatile(p as *mut VirtQueueDesc, desc);
        }
    }

    /// Get available header
    pub fn available_header(&self) -> AvailableHeader {
        unsafe {
            let p = self.pointer.offset(VirtQueue::calc_sizes(self.queue_size as usize).0 as isize);
            ptr::read_volatile(p as *const AvailableHeader)
        }
    }

    /// Set available header
    pub fn set_available_header(&self, header: AvailableHeader) {
        unsafe {
            let p = self.pointer.offset(VirtQueue::calc_sizes(self.queue_size as usize).0 as isize);
            ptr::write_volatile(p as *mut AvailableHeader, header);
        }
    }

    /// Set available ring item
    pub fn set_available_item(&self, index: u16, desc_index: u16) {
        assert!(index < self.queue_size);

        unsafe {
            let p = self.pointer.offset((
                VirtQueue::calc_sizes(self.queue_size as usize).0 +
                mem::size_of::<AvailableHeader>() +
                (index as usize) * sizeof!(AvailableItem)
            ) as isize);
            ptr::write_volatile(p as *mut AvailableItem, AvailableItem { desc_index });
        }
    }
}