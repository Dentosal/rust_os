use alloc::vec::Vec;
use core::mem;
use core::ptr;

use crate::memory::Page;

/// Align upwards to page size
pub fn page_align_up(addr: u64) -> u64 {
    (addr + Page::SIZE - 1) & !(Page::SIZE - 1)
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
    #[rustfmt::skip]
    fn calc_sizes(queue_size: u16) -> (u64, u64, u64) {
        let size_descriptors =     queue_size as u64 * (mem::size_of::<VirtQueueDesc>() as u64);
        let size_available   = 6 + queue_size as u64 * (mem::size_of::<u16>() as u64);
        let size_used        = 6 + queue_size as u64 * (mem::size_of::<UsedItem>() as u64);

        (
            size_descriptors,
            size_available,
            size_used,
        )
    }

    fn calc_size(queue_size: u16) -> u64 {
        let (a, b, c) = VirtQueue::calc_sizes(queue_size);
        page_align_up(a + b + c)
    }

    pub fn new(queue_size: u16) -> VirtQueue {
        let vq_size_bytes = VirtQueue::calc_size(queue_size);
        assert!(vq_size_bytes % Page::SIZE == 0);

        let pointer = {
            unimplemented!("DMA allocator not implmented")
            // DMA_ALLOCATOR
            //     .lock()
            //     .allocate_blocks((vq_size_bytes / Page::SIZE) as usize)
            //     .expect("Could not allocate DMA buffer for VirtIO")
        };

        VirtQueue {
            pointer,
            queue_size,
        }
    }

    pub fn addr(&self) -> usize {
        self.pointer as usize
    }

    pub fn sizes(&self) -> (u64, u64, u64) {
        Self::calc_sizes(self.queue_size)
    }

    pub fn zero(&mut self) {
        // Zeroing means:
        // * empty descriptors
        // * empty available
        // * empty used
        for i in 0..VirtQueue::calc_size(self.queue_size) {
            unsafe {
                ptr::write_volatile(self.pointer.add(i as usize), 0u8);
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
            let p = self
                .pointer
                .offset((index * (mem::size_of::<VirtQueueDesc>() as u16)) as isize);
            ptr::read_volatile(p as *const VirtQueueDesc)
        }
    }

    /// Set descriptor at index
    pub fn set_desc_at(&self, index: u16, desc: VirtQueueDesc) {
        assert!(index < self.queue_size);

        unsafe {
            let p = self
                .pointer
                .add((index * (mem::size_of::<VirtQueueDesc>() as u16)) as usize);
            ptr::write_volatile(p as *mut VirtQueueDesc, desc);
        }
    }

    /// Get available header
    pub fn available_header(&self) -> AvailableHeader {
        unsafe {
            let p = self
                .pointer
                .add(VirtQueue::calc_sizes(self.queue_size).0 as usize);
            ptr::read_volatile(p as *const AvailableHeader)
        }
    }

    /// Set available header
    pub fn set_available_header(&self, header: AvailableHeader) {
        unsafe {
            let p = self
                .pointer
                .add(VirtQueue::calc_sizes(self.queue_size).0 as usize);
            ptr::write_volatile(p as *mut AvailableHeader, header);
        }
    }

    /// Set available ring item
    pub fn set_available_item(&self, index: u16, desc_index: u16) {
        assert!(index < self.queue_size);

        unsafe {
            let p = self.pointer.add(
                (VirtQueue::calc_sizes(self.queue_size).0
                    + (sizeof!(AvailableHeader) as u64)
                    + ((index as u64) * (sizeof!(AvailableItem) as u64))) as usize,
            );
            ptr::write_volatile(p as *mut AvailableItem, AvailableItem { desc_index });
        }
    }
}
